use std::fs;
use std::path::Path;

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
    println!("cargo:rerun-if-changed=src/tray.rs");
    // One list of commands: the ones `src/tray.rs` registers. Each gets an `allow-<name>`
    // permission, and the build fails unless `capabilities/popover.json` grants exactly those,
    // so a new command cannot ship that the popover is not allowed to call.
    let handler = fs::read_to_string("src/tray.rs").expect("reading src/tray.rs");
    let commands = command_grants::registered(&handler).expect("reading the invoke handler");
    let capability =
        fs::read_to_string("capabilities/popover.json").expect("reading capabilities/popover.json");
    if let Err(problem) = command_grants::check_granted(&commands, &capability) {
        panic!("capabilities/popover.json: {problem}");
    }
    let commands: &'static [&'static str] = Vec::leak(
        commands
            .into_iter()
            .map(|name| &*String::leak(name))
            .collect(),
    );
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(commands)),
    )
    .expect("running tauri-build");
}
