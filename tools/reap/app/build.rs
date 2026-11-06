use std::fs;
use std::path::Path;

/// The commands `src/commands.rs` defines; each gets an `allow-<name>`
/// permission, and `capabilities/popover.json` grants them to the popover.
const COMMANDS: &[&str] = &[
    "report",
    "refresh",
    "toggle_pause",
    "pick_folder",
    "add",
    "keep",
    "unkeep",
    "remove_now",
    "stop_process",
    "find_clones",
    "add_suggestion",
    "skip_clone",
    "unskip_clone",
    "add_scan_root",
    "remove_scan_root",
    "open_in_finder",
    "hide",
    "quit",
];

/// Shown when the app was compiled without building `ui/` first.
const PLACEHOLDER: &str = "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>reap</title></head>\
<body><p>The reap UI was not built. Run <code>npm --prefix app/ui ci &amp;&amp; npm --prefix app/ui run build</code>, \
or build the app with <code>cargo tauri build</code>.</p></body></html>\n";

fn main() {
    // `generate_context!` embeds `dist/`, which is build output and not in git.
    // A fresh clone (and `cargo test`, which never runs npm) gets a placeholder
    // page, so the crate compiles without Node; `cargo tauri build` replaces it.
    let dist = Path::new("dist");
    if !dist.join("index.html").exists() {
        fs::create_dir_all(dist).expect("creating app/dist");
        fs::write(dist.join("index.html"), PLACEHOLDER).expect("writing app/dist/index.html");
    }
    // A rebuilt UI must re-embed: rerun (and recompile the crate) when it changes.
    println!("cargo:rerun-if-changed=dist");
    println!("cargo:rerun-if-changed=capabilities");
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("running tauri-build");
}
