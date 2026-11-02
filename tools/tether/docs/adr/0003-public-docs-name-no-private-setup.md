# ADR 0003: public docs name no private setup

- **Status:** Accepted
- **Date:** 2026-10-28

## Context

This repository is going public. Its first docs were written for one operator: they named that
operator's private repositories, linked into them, used them as examples, and recalled an
incident on that machine. An outside reader gets dead links and names of tools they cannot see,
and the operator's setup is published along the way. `CLAUDE.md` § Commits already forbids
private infrastructure and private repository names in new commits; the existing docs predate
that rule.

## Decision

1. **Docs describe roles, not the operator's repositories.** An example clone is
   `~/Development/notes`, a mirror is `~/Mirrors/docs` from `https://github.com/example/docs.git`,
   and the motivating case is "a repository of decisions and runbooks that nothing pulls".
2. **No links into private repositories.** concord, the companion app that keeps one
   instructions repository and `~/.claude` in sync, is described in a sentence where it matters.
   The CI runner is named by its labels, which the workflow needs, without a link.
3. **ADR 0001's context was generalised in place, and only its context.** Its decision, rules
   and considered options are unchanged; it says where it was edited and points here. This is
   the one exception to never editing a decided ADR, because the original text cannot be
   published.
4. **The concord integration stays.** `git tether add` still refuses concord's clone while
   concord is installed (ADR 0001 rule 2). It is behaviour, configured by `CONCORD_REPO_DIR` or
   `TETHER_CONCORD_REPO_DIR`, and harmless where concord is absent.

## Consequences

- The published docs read as a general tool's docs. The removed wording is still in the
  commits before this ADR: making the repository public publishes it there. If that history
  must not be published, publication needs a fresh history, decided at that time.
- New docs follow the same rule; `scripts/check-sensitive.sh` catches paths, secrets and emails
  but not repository names, so review does.
