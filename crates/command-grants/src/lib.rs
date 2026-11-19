//! A Tauri v2 app lists its commands in three places: the invoke handler
//! (`generate_handler!`), the app manifest in `build.rs` (which makes an `allow-<name>`
//! permission for each), and the capability that grants those permissions to a window. A
//! command registered in the handler but missing from the other two compiles, ships, and fails
//! only when clicked, with "Command … not allowed by ACL".
//!
//! Each app's `build.rs` reads the handler's commands with [`registered`], passes them to the
//! manifest, and fails the build with [`check_granted`] unless the capability grants exactly
//! those: one list, kept by the compiler's own source.

use std::collections::BTreeSet;

/// The command names a `generate_handler![…]` in `source` registers, in order: each
/// `commands::<name>` (or bare `<name>`) entry, with any path before the last `::` dropped.
///
/// # Errors
///
/// Fails when `source` has no `generate_handler![` or it registers nothing.
pub fn registered(source: &str) -> Result<Vec<String>, String> {
    let start = source
        .find("generate_handler![")
        .ok_or("no generate_handler![ found")?
        + "generate_handler![".len();
    let end = source[start..]
        .find(']')
        .ok_or("generate_handler![ is not closed")?;
    let names: Vec<String> = source[start..start + end]
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.rsplit("::").next().unwrap_or(entry).to_string())
        .collect();
    if names.is_empty() {
        return Err("generate_handler![ registers no command".into());
    }
    Ok(names)
}

/// Checks that `capability` (a Tauri capability file's JSON) grants `allow-<name>` for every
/// command in `commands` (`_` written as `-`), and grants no `allow-…` for any other: a command
/// it lacks fails at runtime, and a grant for a command that no longer exists is dead.
/// Permissions with a plugin prefix (`core:event:allow-listen`) are not this app's commands
/// and are left alone.
///
/// # Errors
///
/// Fails with every missing and every unknown grant, or when the JSON has no `permissions`.
pub fn check_granted(commands: &[String], capability: &str) -> Result<(), String> {
    let json: serde_json::Value =
        serde_json::from_str(capability).map_err(|e| format!("capability is not JSON: {e}"))?;
    let granted: BTreeSet<String> = json["permissions"]
        .as_array()
        .ok_or("capability has no permissions array")?
        .iter()
        .filter_map(|permission| permission.as_str())
        .filter(|permission| !permission.contains(':'))
        .filter_map(|permission| permission.strip_prefix("allow-"))
        .map(|name| name.replace('-', "_"))
        .collect();
    let wanted: BTreeSet<String> = commands.iter().cloned().collect();
    let missing: Vec<String> = wanted
        .difference(&granted)
        .map(|name| format!("allow-{}", name.replace('_', "-")))
        .collect();
    let unknown: Vec<String> = granted
        .difference(&wanted)
        .map(|name| format!("allow-{}", name.replace('_', "-")))
        .collect();
    let mut problems = Vec::new();
    if !missing.is_empty() {
        problems.push(format!(
            "registered but not granted (the popover gets \"not allowed by ACL\"): {}",
            missing.join(", ")
        ));
    }
    if !unknown.is_empty() {
        problems.push(format!(
            "granted but not registered: {}",
            unknown.join(", ")
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HANDLER: &str = r"
        .invoke_handler(tauri::generate_handler![
            commands::report,
            commands::save_and_remove, // the one that was missed
            quit,
        ])
        .setup(|app| { let x = [1]; })
    ";

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn reads_each_registered_command_and_ignores_comments_and_paths() {
        assert_eq!(
            registered(HANDLER).unwrap(),
            names(&["report", "save_and_remove", "quit"])
        );
        assert!(registered("fn main() {}").is_err());
    }

    #[test]
    fn a_registered_command_the_capability_lacks_fails_the_check() {
        let capability =
            r#"{"permissions": ["core:event:allow-listen", "allow-report", "allow-quit"]}"#;

        let error = check_granted(&registered(HANDLER).unwrap(), capability).unwrap_err();

        assert!(error.contains("allow-save-and-remove"), "{error}");
        assert!(error.contains("not allowed by ACL"), "{error}");
    }

    #[test]
    fn a_grant_for_a_command_that_is_gone_fails_the_check() {
        let capability = r#"{"permissions": ["allow-report", "allow-save-and-remove", "allow-quit", "allow-old"]}"#;

        let error = check_granted(&registered(HANDLER).unwrap(), capability).unwrap_err();

        assert!(
            error.contains("granted but not registered: allow-old"),
            "{error}"
        );
    }

    #[test]
    fn exactly_the_registered_commands_pass_and_plugin_permissions_are_ignored() {
        let capability = r#"{"permissions": ["core:event:allow-listen", "allow-report", "allow-save-and-remove", "allow-quit"]}"#;

        assert_eq!(
            check_granted(&registered(HANDLER).unwrap(), capability),
            Ok(())
        );
    }
}
