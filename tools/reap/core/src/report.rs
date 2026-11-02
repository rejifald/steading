//! `git reap status` and the popover's rows: every linked worktree of every listed repo with
//! its pull request and reap state. Read-only and offline except for the `gh` calls of rule 9
//! (ADR 0002): it judges against the remote-tracking refs as they are, so a stale ref can only
//! make a worktree look unmerged, never merged (rule 8).

use std::fmt::Write as _;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;

use crate::classify::{self, Dirt, Facts, InUse, Merge, State};
use crate::config::{Config, Policy, Repo};
use crate::disk_usage;
use crate::git;
use crate::github::{self, PullRequest, PullRequestState, PullRequests};
use crate::in_use::WorkingDirectories;
use crate::paths::Paths;
use crate::state_file::{self, RepoMemory};
use crate::worktree_list::{self, Worktree};

/// Files in a worktree's git dir that mean an operation is under way (rule 4).
const OPERATIONS: &[(&str, &str)] = &[
    ("MERGE_HEAD", "merge"),
    ("rebase-merge", "rebase"),
    ("rebase-apply", "rebase"),
    ("CHERRY_PICK_HEAD", "cherry-pick"),
    ("REVERT_HEAD", "revert"),
    ("BISECT_LOG", "bisect"),
];

/// The pull request a worktree's branch belongs to, as shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PullRequestSummary {
    pub number: u64,
    pub state: PullRequestState,
    pub url: String,
}

/// One linked worktree.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// The folder's name.
    pub name: String,
    /// With `~`, as every command takes it.
    pub path: String,
    /// The short branch name; absent when detached.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr: Option<PullRequestSummary>,
    pub state: State,
    /// Why it is in that state, in one line.
    pub reason: String,
    /// Its last commit or checkout, RFC 3339.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity: Option<String>,
    /// When a sweep removes it (or prunes its entry), RFC 3339: set once a sweep has started
    /// its grace clock, under `policy = "auto"` only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removes_at: Option<String>,
    /// The disk space removing it would free; absent when the folder is gone or unreadable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

/// One listed repo and its linked worktrees (never the main clone).
#[derive(Debug, Clone, Serialize)]
pub struct RepoReport {
    pub name: String,
    pub path: String,
    /// The base ref as shown, such as `origin/main`.
    pub base: String,
    /// What reap does with this repo's removable worktrees (rule 10).
    pub policy: Policy,
    /// Why `gh` could not be used, so only ancestry merges are seen (rule 9).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gh_note: Option<String>,
    /// Why the repo could not be read at all; `worktrees` is then empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub worktrees: Vec<Row>,
}

/// The tray's three icon states (rule 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Overall {
    Tidy,
    Pending,
    NeedsYou,
}

impl Overall {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Tidy => "tidy",
            Self::Pending => "removal pending",
            Self::NeedsYou => "needs you",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub paused: bool,
    pub overall: Overall,
    /// Removable worktrees and missing entries waiting for a prune.
    pub removable: usize,
    /// Stale worktrees and repos that cannot be read.
    pub needs_you: usize,
    pub repos: Vec<RepoReport>,
}

/// Builds the report for every listed repo. Read-only: the grace clocks and *Keep*s come from
/// `state.json` as the last sweep left them.
///
/// # Errors
///
/// Fails when `repos.toml` cannot be read; a repo that cannot be read only fills its own
/// `error`.
pub fn report(paths: &Paths, now: DateTime<Utc>) -> Result<Report> {
    let repos = Config::load(paths)?.repos(paths)?;
    let memories = state_file::load(paths);
    let directories = WorkingDirectories::read().map_err(|e| format!("{e:#}"));
    let reports = repos
        .iter()
        .map(|repo| {
            let memory = memories
                .get(&paths.tilde(&repo.path))
                .cloned()
                .unwrap_or_default();
            prepare(paths, repo)
                .and_then(|prepared| inspect(paths, repo, &prepared, &directories, &memory, now))
                .map_or_else(
                    |e| unreadable(paths, repo, &e),
                    |inspection| {
                        let mut report = inspection.report;
                        measure(paths, &mut report);
                        report
                    },
                )
        })
        .collect();
    Ok(summarize(paths, reports))
}

/// The totals and overall state of a set of repo reports.
#[must_use]
pub fn summarize(paths: &Paths, repos: Vec<RepoReport>) -> Report {
    let rows = || repos.iter().flat_map(|repo| &repo.worktrees);
    let removable = rows()
        .filter(|row| matches!(row.state, State::Removable | State::Missing))
        .count();
    let needs_you = rows()
        .filter(|row| matches!(row.state, State::Stale | State::Refused))
        .count()
        + repos.iter().filter(|repo| repo.error.is_some()).count();
    let overall = match (needs_you, removable) {
        (0, 0) => Overall::Tidy,
        (0, _) => Overall::Pending,
        _ => Overall::NeedsYou,
    };
    Report {
        paused: paths.is_paused(),
        overall,
        removable,
        needs_you,
        repos,
    }
}

/// The report of a repo that could not be read.
pub(crate) fn unreadable(paths: &Paths, repo: &Repo, error: &anyhow::Error) -> RepoReport {
    RepoReport {
        name: repo.name(),
        path: paths.tilde(&repo.path),
        base: format!("{}/{}", repo.remote, repo.base.as_deref().unwrap_or("?")),
        policy: repo.policy,
        gh_note: None,
        error: Some(format!("{error:#}")),
        worktrees: Vec::new(),
    }
}

/// What a pass needs before it looks at any worktree: the base ref and the pull requests.
pub(crate) struct Prepared {
    /// `refs/remotes/<remote>/<base>`; it may not exist locally until a fetch.
    pub base_ref: String,
    pub pull_requests: Vec<PullRequest>,
    pub gh_note: Option<String>,
    /// `owner/name` when the remote is on GitHub and `gh` answered.
    pub github_repo: Option<String>,
}

impl Prepared {
    /// The base branch's name on the remote.
    pub fn base_branch<'a>(&'a self, repo: &Repo) -> &'a str {
        let prefix = format!("refs/remotes/{}/", repo.remote);
        self.base_ref
            .strip_prefix(&prefix)
            .unwrap_or(&self.base_ref)
    }
}

/// Finds the base ref and asks `gh` for the pull requests.
///
/// # Errors
///
/// Fails when the clone is gone or its default branch is unknown.
pub(crate) fn prepare(paths: &Paths, repo: &Repo) -> Result<Prepared> {
    let base_ref = base_ref(paths, repo)?;
    let url = git::run(&paths.git, &repo.path, &["remote", "get-url", &repo.remote]);
    let github_repo = url.ok().as_deref().and_then(github::github_repo_of);
    let listed = match &github_repo {
        Some(github_repo) => github::list(&paths.gh, &repo.path, github_repo),
        None => PullRequests::Unavailable(format!("{} is not on GitHub", repo.remote)),
    };
    let mut prepared = Prepared {
        gh_note: None,
        pull_requests: Vec::new(),
        github_repo: None,
        base_ref,
    };
    match (listed, github_repo) {
        (PullRequests::Listed(mut pull_requests), Some(github_repo)) => {
            let missed = branches_missed(paths, repo, &pull_requests);
            if !missed.is_empty() {
                pull_requests.extend(github::lookup_heads(
                    &paths.gh,
                    &repo.path,
                    &github_repo,
                    &missed,
                ));
            }
            prepared.pull_requests = pull_requests;
            prepared.github_repo = Some(github_repo);
        }
        (PullRequests::Unavailable(why), _) => {
            prepared.gh_note = Some(format!(
                "{why}: only merges into {} are seen",
                shown(&prepared.base_ref)
            ));
        }
        // `gh` is asked only for a GitHub remote.
        (PullRequests::Listed(_), None) => {}
    }
    Ok(prepared)
}

/// The linked worktrees' branches that no listed pull request has as its head (ADR 0002).
fn branches_missed(paths: &Paths, repo: &Repo, pull_requests: &[PullRequest]) -> Vec<String> {
    let Ok(worktrees) = list_worktrees(paths, &repo.path) else {
        return Vec::new();
    };
    let mut missed: Vec<String> = worktrees
        .iter()
        .skip(1)
        .filter_map(|worktree| worktree.branch.as_deref())
        .map(|branch| branch.strip_prefix("refs/heads/").unwrap_or(branch))
        .filter(|branch| {
            !pull_requests
                .iter()
                .any(|pull_request| pull_request.head_ref_name == *branch)
        })
        .map(str::to_string)
        .collect();
    missed.sort();
    missed.dedup();
    missed
}

/// One linked worktree as a pass sees it.
pub(crate) struct Inspected {
    /// Its path as shown, the key of `state.json`.
    pub key: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub facts: Facts,
    pub state: State,
}

/// A repo's report, with what a sweep needs to act on each row.
pub(crate) struct Inspection {
    pub report: RepoReport,
    pub worktrees: Vec<Inspected>,
}

/// Fills each row's `size_bytes` by walking its folder. Kept out of [`inspect`] so a pass that
/// holds the clone lock never waits on the disk; sizes are display-only.
pub(crate) fn measure(paths: &Paths, report: &mut RepoReport) {
    let folders: Vec<_> = report
        .worktrees
        .iter()
        .map(|row| paths.expand(&row.path))
        .collect();
    for (row, size) in report
        .worktrees
        .iter_mut()
        .zip(disk_usage::measure_all(&folders))
    {
        row.size_bytes = size;
    }
}

/// Looks at every linked worktree of `repo`.
///
/// # Errors
///
/// Fails when the base ref does not exist or `git worktree list` fails.
pub(crate) fn inspect(
    paths: &Paths,
    repo: &Repo,
    prepared: &Prepared,
    directories: &Result<WorkingDirectories, String>,
    memory: &RepoMemory,
    now: DateTime<Utc>,
) -> Result<Inspection> {
    if git::resolve(&paths.git, &repo.path, &prepared.base_ref).is_none() {
        bail!(
            "{} does not exist; fetch it first",
            shown(&prepared.base_ref)
        );
    }
    let worktrees = list_worktrees(paths, &repo.path)?;
    let pass = Pass {
        paths,
        repo,
        base_ref: &prepared.base_ref,
        pull_requests: &prepared.pull_requests,
        github_repo: prepared.github_repo.as_deref(),
        directories,
        memory,
        now,
    };
    // The main worktree comes first and is never removed (rule 2); a bare repo has none.
    let linked: Vec<&Worktree> = worktrees
        .iter()
        .skip(1)
        .filter(|worktree| !worktree.is_bare)
        .collect();
    let (rows, inspected): (Vec<Row>, Vec<Inspected>) =
        linked.iter().map(|worktree| pass.row(worktree)).unzip();
    Ok(Inspection {
        report: RepoReport {
            name: repo.name(),
            path: paths.tilde(&repo.path),
            base: shown(&prepared.base_ref),
            policy: repo.policy,
            gh_note: prepared.gh_note.clone(),
            error: None,
            worktrees: rows,
        },
        worktrees: inspected,
    })
}

/// `refs/remotes/<remote>/<base>`, or the remote's default branch when no base is set.
fn base_ref(paths: &Paths, repo: &Repo) -> Result<String> {
    if !repo.path.is_dir() {
        bail!("{} does not exist", paths.tilde(&repo.path));
    }
    match &repo.base {
        Some(base) => Ok(format!("refs/remotes/{}/{base}", repo.remote)),
        None => git::run(
            &paths.git,
            &repo.path,
            &["symbolic-ref", "--quiet", &format!("refs/remotes/{}/HEAD", repo.remote)],
        )
        .with_context(|| {
            format!(
                "cannot tell {}'s default branch: set `base` in repos.toml, or run `git remote set-head {} --auto`",
                repo.remote, repo.remote
            )
        }),
    }
}

/// `refs/remotes/origin/main` as `origin/main`.
pub(crate) fn shown(refname: &str) -> String {
    refname
        .strip_prefix("refs/remotes/")
        .unwrap_or(refname)
        .to_string()
}

fn list_worktrees(paths: &Paths, repo: &Path) -> Result<Vec<Worktree>> {
    let output = git::run(&paths.git, repo, &["worktree", "list", "--porcelain", "-z"])?;
    // `run` trims the trailing newline only; the NUL terminators survive.
    Ok(worktree_list::parse(&output)?)
}

/// Everything one repo's rows share.
struct Pass<'a> {
    paths: &'a Paths,
    repo: &'a Repo,
    base_ref: &'a str,
    pull_requests: &'a [PullRequest],
    github_repo: Option<&'a str>,
    directories: &'a Result<WorkingDirectories, String>,
    memory: &'a RepoMemory,
    now: DateTime<Utc>,
}

impl Pass<'_> {
    fn row(&self, worktree: &Worktree) -> (Row, Inspected) {
        let branch = worktree.branch.as_deref().map(|branch| {
            branch
                .strip_prefix("refs/heads/")
                .unwrap_or(branch)
                .to_string()
        });
        let head = worktree.head.clone();
        let key = self.paths.tilde(&worktree.path);
        let pull_request = branch
            .as_deref()
            .and_then(|branch| self.pull_request_of(branch, head.as_deref()));
        let mut facts = self.facts(worktree, head.as_deref(), branch.as_deref(), pull_request);
        if let Some(head) = head.as_deref() {
            facts.is_kept = self.memory.is_kept(&key, head);
            facts.refusal = self
                .memory
                .refusal(&key, head)
                .map(|refusal| refusal.message.clone());
        }
        let (state, reason) = classify::classify(
            &facts,
            &shown(self.base_ref),
            self.repo.stale_after,
            self.now,
        );
        let removes_at = matches!(state, State::Removable | State::Missing)
            .then_some(())
            .filter(|()| self.repo.policy == Policy::Auto)
            .and_then(|()| self.memory.clock(&key, head.as_deref().unwrap_or_default()))
            .and_then(|clock| {
                chrono::Duration::from_std(self.repo.grace)
                    .ok()
                    .map(|grace| clock.since + grace)
            });
        let row = Row {
            name: worktree.path.file_name().map_or_else(
                || worktree.path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            ),
            path: key.clone(),
            branch: branch.clone(),
            head: head.clone(),
            pr: pull_request.map(|pull_request| PullRequestSummary {
                number: pull_request.number,
                state: pull_request.state,
                url: pull_request.url.clone(),
            }),
            state,
            reason,
            last_activity: facts.last_activity.map(rfc3339),
            removes_at: removes_at.map(rfc3339),
            size_bytes: None,
        };
        let inspected = Inspected {
            key,
            branch,
            head,
            facts,
            state,
        };
        (row, inspected)
    }

    /// The pull request whose head is `branch`: the one merged or pushed at HEAD, else the
    /// open one, else the newest merged one, else the newest.
    fn pull_request_of(&self, branch: &str, head: Option<&str>) -> Option<&PullRequest> {
        let candidates = || {
            self.pull_requests
                .iter()
                .filter(move |pull_request| pull_request.head_ref_name == branch)
        };
        let in_state = |state| candidates().find(move |pull_request| pull_request.state == state);
        candidates()
            .find(|pull_request| Some(pull_request.head_ref_oid.as_str()) == head)
            .or_else(|| in_state(PullRequestState::Open))
            .or_else(|| in_state(PullRequestState::Merged))
            .or_else(|| candidates().next())
    }

    fn facts(
        &self,
        worktree: &Worktree,
        head: Option<&str>,
        branch: Option<&str>,
        pull_request: Option<&PullRequest>,
    ) -> Facts {
        let is_missing = worktree.prunable_reason.is_some() || !worktree.path.is_dir();
        let mut facts = Facts {
            is_missing,
            lock_reason: worktree.lock_reason.clone(),
            ..Facts::default()
        };
        if is_missing {
            return facts;
        }
        let state = pull_request.map(|pull_request| pull_request.state);
        facts.closed_pull_request = pull_request
            .filter(|_| state == Some(PullRequestState::Closed))
            .map(|pull_request| pull_request.number);
        facts.open_pull_request = pull_request
            .filter(|_| state == Some(PullRequestState::Open))
            .map(|pull_request| pull_request.number);
        facts.last_activity = self.last_activity(&worktree.path);
        facts.merge = head.and_then(|head| self.merge_of(head, branch, pull_request));
        if facts.merge.is_none() && facts.open_pull_request.is_none() && branch.is_some() {
            facts.pushed_to = head
                .filter(|head| self.is_pushed(head))
                .map(|_| self.repo.remote.clone());
        }
        if facts.merge.is_some() || facts.pushed_to.is_some() {
            facts.dirt = self.dirt_of(&worktree.path);
            facts.in_use = match self.directories {
                Ok(directories) if directories.is_inside(&worktree.path) => InUse::Yes,
                Ok(_) => InUse::No,
                Err(why) => InUse::Unknown(why.clone()),
            };
        }
        facts
    }

    /// Rule 3, clauses (a), (c) and (b) in that order.
    fn merge_of(
        &self,
        head: &str,
        branch: Option<&str>,
        pull_request: Option<&PullRequest>,
    ) -> Option<Merge> {
        let git = &self.paths.git;
        let repo = &self.repo.path;
        if git::is_ancestor(git, repo, head, self.base_ref) {
            return Some(Merge::IntoBase(shown(self.base_ref)));
        }
        let pull_request = pull_request.filter(|_| branch.is_some())?;
        if pull_request.state == PullRequestState::Merged
            && self.is_contained(head, &pull_request.head_ref_oid)
        {
            return Some(Merge::PullRequest(pull_request.number));
        }
        let target = format!(
            "refs/remotes/{}/{}",
            self.repo.remote, pull_request.base_ref_name
        );
        let is_merged_into_target = git::resolve(git, repo, &target).is_some()
            && git::is_ancestor(git, repo, head, &target);
        is_merged_into_target.then(|| Merge::IntoTarget {
            target: shown(&target),
            pull_request: pull_request.number,
        })
    }

    /// ADR 0003: every commit on `head` is in the remote's tracking refs.
    fn is_pushed(&self, head: &str) -> bool {
        let remotes = format!("--remotes={}", self.repo.remote);
        git::run(
            &self.paths.git,
            &self.repo.path,
            &["rev-list", "--count", head, "--not", &remotes],
        )
        .is_ok_and(|count| count.trim() == "0")
    }

    /// Rule 3(c) as ADR 0002 amends it: HEAD is the merged head or an ancestor of it. Checked in
    /// the clone when it has the merged head, and asked of GitHub only when it does not.
    fn is_contained(&self, head: &str, merged_head: &str) -> bool {
        if head == merged_head {
            return true;
        }
        let git = &self.paths.git;
        let repo = &self.repo.path;
        if git::resolve(git, repo, merged_head).is_some() {
            return git::is_ancestor(git, repo, head, merged_head);
        }
        self.github_repo.is_some_and(|github_repo| {
            github::contains(&self.paths.gh, repo, github_repo, head, merged_head)
        })
    }

    /// Rule 4: an operation under way, or any change `git status` reports (untracked files
    /// count, ignored ones do not).
    fn dirt_of(&self, worktree: &Path) -> Option<Dirt> {
        let git_dir = git::path_of(
            &self.paths.git,
            worktree,
            &["rev-parse", "--absolute-git-dir"],
        );
        let Ok(git_dir) = git_dir else {
            // A worktree git cannot read is not known to be clean.
            return Some(Dirt::Changes);
        };
        if let Some((_, operation)) = OPERATIONS
            .iter()
            .find(|(file, _)| git_dir.join(file).exists())
        {
            return Some(Dirt::Operation(operation));
        }
        match git::run(
            &self.paths.git,
            worktree,
            &["status", "--porcelain", "--untracked-files=normal"],
        ) {
            Ok(status) if status.is_empty() => None,
            _ => Some(Dirt::Changes),
        }
    }

    /// The later of the last commit and the last change to the worktree's reflog (a commit,
    /// checkout or reset), both of which mean someone worked there (rule 13).
    fn last_activity(&self, worktree: &Path) -> Option<DateTime<Utc>> {
        let committed = git::run(
            &self.paths.git,
            worktree,
            &["log", "-1", "--format=%ct", "HEAD"],
        )
        .ok()
        .and_then(|seconds| seconds.parse::<i64>().ok())
        .and_then(|seconds| DateTime::from_timestamp(seconds, 0));
        let checked_out = git::path_of(
            &self.paths.git,
            worktree,
            &["rev-parse", "--git-path", "logs/HEAD"],
        )
        .ok()
        .and_then(|log| std::fs::metadata(log).ok()?.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|since| DateTime::from_timestamp(i64::try_from(since.as_secs()).ok()?, 0));
        committed.max(checked_out)
    }
}

fn rfc3339(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Human-readable `status`.
#[must_use]
pub fn render(report: &Report) -> String {
    let mut out = format!("git reap: {}", report.overall.label());
    if report.paused {
        out.push_str(" (paused)");
    }
    out.push('\n');
    if let Some(sizes) = sizes_line(report) {
        let _ = writeln!(out, "  {sizes}");
    }
    if report.repos.is_empty() {
        out.push_str("  no repos listed; add a main clone with `git reap add <path>`\n");
    }
    for repo in &report.repos {
        let _ = writeln!(
            out,
            "{}  ({}, policy {})",
            repo.path,
            repo.base,
            repo.policy.label()
        );
        if let Some(error) = &repo.error {
            let _ = writeln!(out, "  cannot read: {error}");
        }
        if let Some(note) = &repo.gh_note {
            let _ = writeln!(out, "  {note}");
        }
        if repo.error.is_none() && repo.worktrees.is_empty() {
            out.push_str("  no linked worktrees\n");
        }
        for row in &repo.worktrees {
            let branch = row.branch.as_deref().unwrap_or("(detached)");
            let pull_request = row
                .pr
                .as_ref()
                .map(|pull_request| format!(" #{}", pull_request.number))
                .unwrap_or_default();
            let removes = row
                .removes_at
                .as_deref()
                .and_then(|time| DateTime::parse_from_rfc3339(time).ok())
                .map(|time| {
                    format!(
                        "; removes at {}",
                        time.with_timezone(&chrono::Local).format("%a %-d %b %H:%M")
                    )
                })
                .unwrap_or_default();
            let size = row
                .size_bytes
                .map(|bytes| format!("  {}", shown_size(bytes)))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "  {:<10} {}  {branch}{pull_request}{size}\n             {}{removes}",
                row.state.label(),
                row.path,
                row.reason
            );
        }
    }
    out
}

/// "worktrees take 14 GB: 3.2 GB removable, 1.1 GB held by uncommitted changes", leaving out
/// a zero part; `None` when nothing was measured.
fn sizes_line(report: &Report) -> Option<String> {
    let rows: Vec<&Row> = report
        .repos
        .iter()
        .flat_map(|repo| &repo.worktrees)
        .collect();
    let total_of = |states: &[State]| -> u64 {
        rows.iter()
            .filter(|row| states.contains(&row.state))
            .filter_map(|row| row.size_bytes)
            .sum()
    };
    let total: u64 = rows.iter().filter_map(|row| row.size_bytes).sum();
    if total == 0 {
        return None;
    }
    let parts: Vec<String> = [
        (total_of(&[State::Removable]), "removable"),
        (total_of(&[State::Dirty]), "held by uncommitted changes"),
    ]
    .into_iter()
    .filter(|(bytes, _)| *bytes > 0)
    .map(|(bytes, label)| format!("{} {label}", shown_size(bytes)))
    .collect();
    let mut line = format!("worktrees take {}", shown_size(total));
    if !parts.is_empty() {
        let _ = write!(line, ": {}", parts.join(", "));
    }
    Some(line)
}

/// Bytes in decimal units as Finder shows them: "840 MB", "3.2 GB", "14 GB".
#[must_use]
pub fn shown_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["bytes", "kB", "MB", "GB", "TB"];
    const STEP: f64 = 1000.0;
    // Precision lost above 2^52 bytes (4 PB) does not show at one decimal.
    #[allow(clippy::cast_precision_loss)]
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= STEP && unit < UNITS.len() - 1 {
        value /= STEP;
        unit += 1;
    }
    if unit == 0 || value >= 10.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_sizes_in_decimal_units() {
        assert_eq!(shown_size(512), "512 bytes");
        assert_eq!(shown_size(840_000_000), "840 MB");
        assert_eq!(shown_size(3_240_000_000), "3.2 GB");
        assert_eq!(shown_size(14_400_000_000), "14 GB");
    }
}
