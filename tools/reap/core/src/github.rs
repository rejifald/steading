//! Pull requests from `gh` (rule 9): one batched `gh pr list --json` per repo per pass, a
//! GraphQL lookup for the worktree branches it missed, and a compare call for a merged head the
//! clone lacks (ADR 0002), and a GraphQL lookup of the pull requests associated with the HEADs
//! that are still unmerged (ADR 0006). `gh` is optional; without it, reap sees ancestry merges (rule 3a)
//! only, and says why.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

/// How many pull requests one pass reads, newest first. Only those whose head is a worktree's
/// branch matter, and a worktree's pull request is almost always among the recent ones.
const PULL_REQUEST_LIMIT: &str = "200";
const FIELDS: &str = "number,state,headRefName,headRefOid,baseRefName,url";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PullRequestState {
    #[serde(alias = "OPEN")]
    Open,
    #[serde(alias = "MERGED")]
    Merged,
    #[serde(alias = "CLOSED")]
    Closed,
}

/// One pull request as `gh pr list --json` spells it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u64,
    pub state: PullRequestState,
    pub head_ref_name: String,
    /// The head commit when the pull request last changed; for a merged one, what was merged.
    pub head_ref_oid: String,
    pub base_ref_name: String,
    pub url: String,
}

/// The pull requests of one repo, or why there are none to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullRequests {
    Listed(Vec<PullRequest>),
    Unavailable(String),
}

/// `owner/name` on github.com from a remote URL in any of git's forms; `None` for any other
/// host.
#[must_use]
pub fn github_repo_of(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = [
        "https://github.com/",
        "http://github.com/",
        "ssh://git@github.com/",
        "git@github.com:",
    ]
    .iter()
    .find_map(|prefix| url.strip_prefix(prefix))?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let (owner, name) = rest.split_once('/')?;
    let is_plain = |part: &str| !part.is_empty() && !part.contains('/');
    (is_plain(owner) && is_plain(name)).then(|| format!("{owner}/{name}"))
}

/// Lists the pull requests of `repo` (`owner/name`) with `gh`, run in `dir`.
pub fn list(gh: &OsStr, dir: &Path, repo: &str) -> PullRequests {
    let output = Command::new(gh)
        .current_dir(dir)
        .args(["pr", "list", "--repo", repo, "--state", "all"])
        .args(["--limit", PULL_REQUEST_LIMIT, "--json", FIELDS])
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .output();
    let output = match output {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return PullRequests::Unavailable("gh is not installed".into());
        }
        Err(e) => return PullRequests::Unavailable(format!("gh could not start: {e}")),
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("gh auth login") {
            return PullRequests::Unavailable("gh is not signed in".into());
        }
        let line = stderr.lines().map(str::trim).find(|line| !line.is_empty());
        return PullRequests::Unavailable(format!(
            "gh failed: {}",
            line.unwrap_or("no error output")
        ));
    }
    match serde_json::from_slice(&output.stdout) {
        Ok(pull_requests) => PullRequests::Listed(pull_requests),
        Err(e) => PullRequests::Unavailable(format!("gh answered something unexpected: {e}")),
    }
}

/// How many head branches one GraphQL lookup asks about.
const LOOKUP_BATCH: usize = 100;
const NODE_FIELDS: &str = "number state headRefName headRefOid baseRefName url";

/// The pull requests whose head is one of `branches`, which the newest-first list missed
/// (ADR 0002, rule 3): one GraphQL call per [`LOOKUP_BATCH`] branches. A failed call finds
/// nothing, which can only keep a worktree unmerged.
#[must_use]
pub fn lookup_heads(gh: &OsStr, dir: &Path, repo: &str, branches: &[String]) -> Vec<PullRequest> {
    let Some((owner, name)) = repo.split_once('/') else {
        return Vec::new();
    };
    branches
        .chunks(LOOKUP_BATCH)
        .flat_map(|chunk| {
            let query = lookup_query(owner, name, chunk);
            api(gh, dir, &["graphql", "-f", &format!("query={query}")])
                .map(|stdout| parse_lookup(&stdout))
                .unwrap_or_default()
        })
        .collect()
}

fn lookup_query(owner: &str, name: &str, branches: &[String]) -> String {
    let literal = |text: &str| serde_json::to_string(text).unwrap_or_default();
    let mut fields = String::new();
    for (index, branch) in branches.iter().enumerate() {
        let _ = write!(
            fields,
            "b{index}: pullRequests(headRefName: {}, first: 5, states: [OPEN, CLOSED, MERGED], \
             orderBy: {{field: CREATED_AT, direction: DESC}}) {{ nodes {{ {NODE_FIELDS} }} }} ",
            literal(branch)
        );
    }
    format!(
        "query {{ repository(owner: {}, name: {}) {{ {fields}}} }}",
        literal(owner),
        literal(name)
    )
}

fn parse_lookup(stdout: &[u8]) -> Vec<PullRequest> {
    #[derive(Deserialize)]
    struct Nodes {
        nodes: Vec<PullRequest>,
    }
    #[derive(Deserialize)]
    struct Data {
        repository: BTreeMap<String, Nodes>,
    }
    #[derive(Deserialize)]
    struct Response {
        data: Data,
    }
    serde_json::from_slice::<Response>(stdout)
        .map(|response| {
            response
                .data
                .repository
                .into_values()
                .flat_map(|nodes| nodes.nodes)
                .collect()
        })
        .unwrap_or_default()
}

/// The pull requests GitHub associates with each of `heads` (full SHAs), whatever their branch
/// (ADR 0006, rule 1): one GraphQL call per [`LOOKUP_BATCH`] commits, `object(oid:)` with
/// `associatedPullRequests`. A commit GitHub does not have comes back `null` and maps to no pull
/// request, as does every commit of a failed call; neither can make a worktree merged. Never
/// GitHub's search, which also matches a SHA quoted in a pull request's text.
#[must_use]
pub fn associated(
    gh: &OsStr,
    dir: &Path,
    repo: &str,
    heads: &[String],
) -> BTreeMap<String, Vec<PullRequest>> {
    let mut found: BTreeMap<String, Vec<PullRequest>> = BTreeMap::new();
    let Some((owner, name)) = repo.split_once('/') else {
        return found;
    };
    // `object(oid:)` needs the full object name; anything else is an error for the whole call.
    let heads: Vec<&String> = heads.iter().filter(|head| is_full_sha(head)).collect();
    for chunk in heads.chunks(LOOKUP_BATCH) {
        let query = associated_query(owner, name, chunk);
        let answer = api(gh, dir, &["graphql", "-f", &format!("query={query}")])
            .map(|stdout| parse_associated(&stdout))
            .unwrap_or_default();
        for (index, head) in chunk.iter().enumerate() {
            let pull_requests = answer.get(&index).cloned().unwrap_or_default();
            found.insert((*head).clone(), pull_requests);
        }
    }
    found
}

/// 40 hex digits (SHA-1) or 64 (SHA-256).
fn is_full_sha(text: &str) -> bool {
    matches!(text.len(), 40 | 64) && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn associated_query(owner: &str, name: &str, heads: &[&String]) -> String {
    let literal = |text: &str| serde_json::to_string(text).unwrap_or_default();
    let mut fields = String::new();
    for (index, head) in heads.iter().enumerate() {
        let _ = write!(
            fields,
            "c{index}: object(oid: {}) {{ ... on Commit {{ associatedPullRequests(first: 5) \
             {{ nodes {{ {NODE_FIELDS} }} }} }} }} ",
            literal(head)
        );
    }
    format!(
        "query {{ repository(owner: {}, name: {}) {{ {fields}}} }}",
        literal(owner),
        literal(name)
    )
}

/// The pull requests of each `c<index>` alias; a `null` commit has none.
fn parse_associated(stdout: &[u8]) -> BTreeMap<usize, Vec<PullRequest>> {
    #[derive(Deserialize)]
    struct Nodes {
        nodes: Vec<PullRequest>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Commit {
        associated_pull_requests: Option<Nodes>,
    }
    #[derive(Deserialize)]
    struct Data {
        repository: BTreeMap<String, Option<Commit>>,
    }
    #[derive(Deserialize)]
    struct Response {
        data: Data,
    }
    let Ok(response) = serde_json::from_slice::<Response>(stdout) else {
        return BTreeMap::new();
    };
    response
        .data
        .repository
        .into_iter()
        .filter_map(|(alias, commit)| {
            let index = alias.strip_prefix('c')?.parse().ok()?;
            let nodes = commit?.associated_pull_requests?.nodes;
            Some((index, nodes))
        })
        .collect()
}

/// Whether GitHub holds `ancestor` as an ancestor of, or equal to, `of` (ADR 0002, rule 1). A
/// commit GitHub does not have, a failed call or any other answer is `false`.
#[must_use]
pub fn contains(gh: &OsStr, dir: &Path, repo: &str, ancestor: &str, of: &str) -> bool {
    let endpoint = format!("repos/{repo}/compare/{ancestor}...{of}");
    api(gh, dir, &[&endpoint, "--jq", ".status"]).is_some_and(|stdout| {
        matches!(
            String::from_utf8_lossy(&stdout).trim(),
            "ahead" | "identical"
        )
    })
}

/// `gh api <args>`'s stdout, or `None` when it fails.
fn api(gh: &OsStr, dir: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new(gh)
        .current_dir(dir)
        .arg("api")
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lookup_asks_for_each_branch_as_a_string_literal() {
        let query = lookup_query("acme", "app", &["feat/a".into(), "say \"hi\"".into()]);

        assert!(query.starts_with(r#"query { repository(owner: "acme", name: "app") { b0: "#));
        assert!(query.contains(r#"b0: pullRequests(headRefName: "feat/a""#));
        assert!(query.contains(r#"b1: pullRequests(headRefName: "say \"hi\"""#));
    }

    #[test]
    fn parses_a_lookup_and_ignores_anything_else() {
        let json = br#"{"data":{"repository":{"b0":{"nodes":[{"number":7,"state":"MERGED",
            "headRefName":"feat/a","headRefOid":"abc","baseRefName":"main","url":"u"}]},
            "b1":{"nodes":[]}}}}"#;

        let found = parse_lookup(json);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].number, 7);
        assert_eq!(parse_lookup(b"[]"), Vec::new());
    }

    #[test]
    fn an_associated_lookup_asks_for_each_full_sha_by_object() {
        let sha = "0123456789abcdef0123456789abcdef01234567".to_string();
        let query = associated_query("acme", "app", &[&sha]);

        assert!(query.contains(&format!(
            r#"c0: object(oid: "{sha}") {{ ... on Commit {{ associatedPullRequests(first: 5)"#
        )));
        assert!(is_full_sha(&sha));
        assert!(!is_full_sha("0123456"));
        assert!(!is_full_sha("main\") { x }"));
    }

    #[test]
    fn parses_an_associated_lookup_with_a_null_commit() {
        let json = br#"{"data":{"repository":{"c0":null,"c1":{"associatedPullRequests":{"nodes":[
            {"number":212,"state":"MERGED","headRefName":"batch","headRefOid":"abc",
            "baseRefName":"main","url":"u"}]}},"c2":{}}}}"#;

        let found = parse_associated(json);

        assert_eq!(found.get(&0), None);
        assert_eq!(found[&1][0].number, 212);
        assert_eq!(found.get(&2), None);
        assert!(parse_associated(b"{}").is_empty());
    }

    #[test]
    fn reads_owner_and_name_from_every_github_url_form() {
        for url in [
            "https://github.com/acme/app.git",
            "https://github.com/acme/app",
            "git@github.com:acme/app.git",
            "ssh://git@github.com/acme/app.git",
        ] {
            assert_eq!(github_repo_of(url).as_deref(), Some("acme/app"), "{url}");
        }
    }

    #[test]
    fn other_hosts_and_local_paths_are_not_github() {
        for url in [
            "https://gitlab.com/acme/app.git",
            "/srv/git/app.git",
            "https://github.com/acme",
        ] {
            assert_eq!(github_repo_of(url), None, "{url}");
        }
    }

    #[test]
    fn parses_gh_json() {
        let json = r#"[{"number":12,"state":"MERGED","headRefName":"feat/a","headRefOid":"abc",
            "baseRefName":"main","url":"https://github.com/acme/app/pull/12"}]"#;

        let pull_requests: Vec<PullRequest> = serde_json::from_str(json).unwrap();

        assert_eq!(pull_requests[0].state, PullRequestState::Merged);
        assert_eq!(pull_requests[0].base_ref_name, "main");
    }
}
