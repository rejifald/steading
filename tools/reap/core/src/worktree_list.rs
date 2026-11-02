//! Reads `git worktree list --porcelain -z`, the source of every worktree reap sees (rule 2).
//!
//! `-z` separates attributes with NUL instead of newline, so a path containing a newline
//! survives. Each worktree is a run of attributes ended by an empty one.

use std::fmt;
use std::path::PathBuf;

/// One entry of `git worktree list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    /// The commit checked out; `None` for a bare repository.
    pub head: Option<String>,
    /// The full ref checked out (`refs/heads/<name>`); `None` when detached or bare.
    pub branch: Option<String>,
    pub is_bare: bool,
    pub is_detached: bool,
    /// `Some` when `git worktree lock`ed, holding the reason (empty when none was given).
    pub lock_reason: Option<String>,
    /// `Some` when git considers the entry prunable, holding why (its folder is gone).
    pub prunable_reason: Option<String>,
}

/// Output that does not start a worktree before giving it attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub attribute: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "worktree list: attribute {:?} before any `worktree` line",
            self.attribute
        )
    }
}

impl std::error::Error for ParseError {}

/// Parses the output of `git worktree list --porcelain -z`, main worktree first.
///
/// Attributes git may add later are ignored, so a newer git does not break the sweep.
///
/// # Errors
///
/// Returns [`ParseError`] when an attribute appears before the first `worktree` line.
pub fn parse(output: &str) -> Result<Vec<Worktree>, ParseError> {
    let mut worktrees = Vec::new();
    let mut current: Option<Worktree> = None;
    for attribute in output.split('\0') {
        if attribute.is_empty() {
            worktrees.extend(current.take());
            continue;
        }
        let (key, value) = attribute.split_once(' ').unwrap_or((attribute, ""));
        if key == "worktree" {
            worktrees.extend(current.take());
            current = Some(Worktree::at(value));
            continue;
        }
        let Some(worktree) = current.as_mut() else {
            return Err(ParseError {
                attribute: attribute.to_owned(),
            });
        };
        match key {
            "HEAD" => worktree.head = Some(value.to_owned()),
            "branch" => worktree.branch = Some(value.to_owned()),
            "bare" => worktree.is_bare = true,
            "detached" => worktree.is_detached = true,
            "locked" => worktree.lock_reason = Some(value.to_owned()),
            "prunable" => worktree.prunable_reason = Some(value.to_owned()),
            _ => {}
        }
    }
    worktrees.extend(current);
    Ok(worktrees)
}

impl Worktree {
    fn at(path: &str) -> Self {
        Self {
            path: PathBuf::from(path),
            head: None,
            branch: None,
            is_bare: false,
            is_detached: false,
            lock_reason: None,
            prunable_reason: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "d60199f0a863885e137d9040a40761348ff32609";

    fn record(attributes: &[&str]) -> String {
        let mut out = attributes.join("\0");
        out.push_str("\0\0");
        out
    }

    #[test]
    fn reads_main_and_linked_worktrees_in_order() {
        let output = [
            record(&[
                "worktree /Users/me/repo",
                &format!("HEAD {SHA}"),
                "branch refs/heads/main",
            ]),
            record(&[
                "worktree /Users/me/feat",
                &format!("HEAD {SHA}"),
                "branch refs/heads/feat",
            ]),
        ]
        .concat();

        let worktrees = parse(&output).unwrap();

        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].path, PathBuf::from("/Users/me/repo"));
        assert_eq!(worktrees[1].branch.as_deref(), Some("refs/heads/feat"));
        assert_eq!(worktrees[1].head.as_deref(), Some(SHA));
    }

    #[test]
    fn reads_detached_locked_and_prunable_entries() {
        let output = [
            record(&[
                "worktree /Users/me/repo",
                &format!("HEAD {SHA}"),
                "branch refs/heads/main",
            ]),
            record(&[
                "worktree /Users/me/usb",
                &format!("HEAD {SHA}"),
                "detached",
                "locked on usb",
            ]),
            record(&[
                "worktree /Users/me/gone",
                &format!("HEAD {SHA}"),
                "branch refs/heads/gone",
                "prunable gitdir file points to non-existent location",
            ]),
        ]
        .concat();

        let worktrees = parse(&output).unwrap();

        assert!(worktrees[1].is_detached);
        assert_eq!(worktrees[1].branch, None);
        assert_eq!(worktrees[1].lock_reason.as_deref(), Some("on usb"));
        assert_eq!(
            worktrees[2].prunable_reason.as_deref(),
            Some("gitdir file points to non-existent location")
        );
    }

    #[test]
    fn a_lock_without_reason_is_still_locked() {
        let output = record(&["worktree /Users/me/repo", "bare", "locked"]);

        let worktrees = parse(&output).unwrap();

        assert!(worktrees[0].is_bare);
        assert_eq!(worktrees[0].lock_reason.as_deref(), Some(""));
    }

    #[test]
    fn keeps_a_path_containing_a_newline() {
        let output = record(&[
            "worktree /Users/me/odd\nname",
            &format!("HEAD {SHA}"),
            "detached",
        ]);

        let worktrees = parse(&output).unwrap();

        assert_eq!(worktrees[0].path, PathBuf::from("/Users/me/odd\nname"));
    }

    #[test]
    fn ignores_attributes_it_does_not_know() {
        let output = record(&["worktree /Users/me/repo", "someday-new value", "detached"]);

        let worktrees = parse(&output).unwrap();

        assert!(worktrees[0].is_detached);
    }

    #[test]
    fn empty_output_has_no_worktrees() {
        assert_eq!(parse("").unwrap(), Vec::new());
    }

    #[test]
    fn refuses_an_attribute_before_any_worktree() {
        let error = parse(&record(&[&format!("HEAD {SHA}")])).unwrap_err();

        assert_eq!(error.attribute, format!("HEAD {SHA}"));
    }
}
