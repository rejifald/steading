//! Notifications and the folder picker through `osascript`, as in git-tether, so the app needs
//! no Tauri dialog or notification plugin.

use std::process::Command;

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn run(script: &str) -> Option<String> {
    let out = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

pub fn notify(title: &str, message: &str) {
    run(&format!(
        "display notification {} with title {}",
        quote(message),
        quote(title)
    ));
}

/// The macOS folder picker; `None` when cancelled. Blocks until answered.
pub fn choose_folder() -> Option<String> {
    let folder = run(
        "POSIX path of (choose folder with prompt \"Choose a main clone for reap to keep tidy\")",
    )?;
    Some(folder.trim_end_matches('/').to_string())
}

#[cfg(test)]
mod tests {
    use super::quote;

    #[test]
    fn quotes_survive_applescript() {
        assert_eq!(quote(r#"a "b" \c"#), r#""a \"b\" \\c""#);
    }
}
