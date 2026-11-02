//! Fetch only what moved (rule 8): one `git ls-remote` for the refs a pass judges against (the
//! base, and the target of every pull request whose head is a local branch), then one fetch of
//! those whose remote tip differs from the remote-tracking ref. Only remote-tracking refs are
//! written; a failure leaves them as they were, which can only delay a removal.

use std::collections::{BTreeMap, BTreeSet};

use crate::config::Repo;
use crate::ctx::Ctx;
use crate::git;
use crate::report::Prepared;

/// Fetches the refs that moved; `Some(note)` when the remote could not be read or fetched.
pub(crate) fn fetch_moved(ctx: &Ctx, repo: &Repo, prepared: &Prepared) -> Option<String> {
    let branches = branches_to_check(ctx, repo, prepared);
    let remote_tips = match remote_tips(ctx, repo, &branches) {
        Ok(tips) => tips,
        Err(why) => {
            return Some(format!(
                "could not reach {}: {why}; judged against the refs as they were",
                repo.remote
            ));
        }
    };
    let refspecs: Vec<String> = remote_tips
        .iter()
        .filter(|(branch, tip)| {
            let tracking = format!("refs/remotes/{}/{branch}", repo.remote);
            git::resolve(&ctx.paths.git, &repo.path, &tracking).as_deref() != Some(tip.as_str())
        })
        .map(|(branch, _)| format!("+refs/heads/{branch}:refs/remotes/{}/{branch}", repo.remote))
        .collect();
    if refspecs.is_empty() {
        return None;
    }
    let mut args = vec![
        "fetch",
        "--quiet",
        "--no-tags",
        "--no-write-fetch-head",
        &repo.remote,
    ];
    args.extend(refspecs.iter().map(String::as_str));
    ctx.write(&repo.path, &args)
        .err()
        .map(|e| format!("fetch failed: {e:#}; judged against the refs as they were"))
}

/// The base branch and every pull request target whose head is a local branch.
fn branches_to_check(ctx: &Ctx, repo: &Repo, prepared: &Prepared) -> BTreeSet<String> {
    let local: BTreeSet<String> = git::run(
        &ctx.paths.git,
        &repo.path,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads/"],
    )
    .map(|out| out.lines().map(str::to_string).collect())
    .unwrap_or_default();
    let mut branches = BTreeSet::from([prepared.base_branch(repo).to_string()]);
    branches.extend(
        prepared
            .pull_requests
            .iter()
            .filter(|pull_request| local.contains(&pull_request.head_ref_name))
            .map(|pull_request| pull_request.base_ref_name.clone()),
    );
    branches
}

/// Each branch's tip on the remote; a branch the remote no longer has is left out.
fn remote_tips(
    ctx: &Ctx,
    repo: &Repo,
    branches: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>, String> {
    let refnames: Vec<String> = branches
        .iter()
        .map(|branch| format!("refs/heads/{branch}"))
        .collect();
    let mut args = vec!["ls-remote", "--quiet", repo.remote.as_str()];
    args.extend(refnames.iter().map(String::as_str));
    let output = git::output(&ctx.paths.git, &repo.path, &args).map_err(|e| format!("{e:#}"))?;
    if !output.status.success() {
        return Err(git::first_error_line(&output));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter_map(|(tip, refname)| {
            Some((
                refname.strip_prefix("refs/heads/")?.to_string(),
                tip.to_string(),
            ))
        })
        .filter(|(branch, _)| branches.contains(branch))
        .collect())
}
