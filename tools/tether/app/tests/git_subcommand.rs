//! `git tether <cmd>`: git runs `git-tether <cmd>` from PATH. The installer links that name
//! to the app binary, so the app must run the CLI whatever name it was started under.

use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::process::Command;

/// A fresh directory holding a `git-tether` link to the app binary.
fn linked_binary(test: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("git-tether-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let link = dir.join("git-tether");
    symlink(env!("CARGO_BIN_EXE_tether"), &link).unwrap();
    (dir, link)
}

#[test]
fn a_git_tether_link_to_the_app_binary_runs_the_cli() {
    let (dir, link) = linked_binary("status");

    let out = Command::new(&link)
        .arg("status")
        .env("TETHER_CONFIG_DIR", dir.join("config"))
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("git tether:"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn usage_names_the_git_subcommand() {
    let (dir, link) = linked_binary("help");

    let out = Command::new(&link).arg("--help").output().unwrap();

    assert!(String::from_utf8_lossy(&out.stdout).contains("git tether"));
    std::fs::remove_dir_all(dir).unwrap();
}
