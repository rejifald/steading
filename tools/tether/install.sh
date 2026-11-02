#!/bin/sh
# Builds tether and installs it on this Mac (ADR 0001 rule 12):
#   1. cargo tauri build  -> tether.app (unsigned; a local build needs none), which
#      builds the popover UI in app/ui with npm first
#   2. show `git tether status` and the first `git tether sync --dry-run` plan, and ask
#      to confirm; on "no" nothing is installed
#   3. copy to ~/Applications, link the CLI to ~/.local/bin/git-tether
#   4. register the login item (a LaunchAgent) and start it
#
#   ./install.sh              # build, plan, confirm, install, start
#   ./install.sh --dry-run    # print the steps, change nothing
#   ./install.sh --yes        # no confirmation prompt
#   ./install.sh --uninstall  # remove the login item, link, app
#
# It runs as you, with your git credentials. --uninstall leaves
# ~/.config/tether (repos.toml, backups) and every listed clone alone.
set -eu

dry=0 yes=0 uninstall=0
for a in "$@"; do
  case "$a" in
    --dry-run) dry=1 ;;
    --yes|-y) yes=1 ;;
    --uninstall) uninstall=1 ;;
    *) echo "install.sh: unknown option $a" >&2; exit 2 ;;
  esac
done

here=$(cd "$(dirname "$0")" && pwd -P)
root=$(cd "$here/../.." && pwd -P)
label=io.github.rejifald.tether
apps="$HOME/Applications"
app="$apps/tether.app"
bin="$app/Contents/MacOS/tether"
link="$HOME/.local/bin/git-tether"
agent="$HOME/Library/LaunchAgents/$label.plist"
domain="gui/$(id -u)"

run() {
  echo "+ $*"
  [ "$dry" = 1 ] || "$@"
}

bootout() {
  if launchctl print "$domain/$label" >/dev/null 2>&1; then
    run launchctl bootout "$domain/$label"
  fi
}

if [ "$uninstall" = 1 ]; then
  echo "== login item $agent"
  bootout
  if [ -e "$agent" ]; then run rm -f "$agent"; fi
  echo "== CLI link $link"
  if [ -L "$link" ] && [ "$(readlink "$link")" = "$bin" ]; then
    run rm -f "$link"
  elif [ -e "$link" ] || [ -L "$link" ]; then
    echo "left alone: $link is not git-tether's link"
  fi
  echo "== app $app"
  if [ -e "$app" ]; then run rm -rf "$app"; fi
  echo "done; ~/.config/tether and the listed clones are untouched"
  exit 0
fi

command -v cargo >/dev/null || { echo "install.sh: needs a Rust toolchain (https://rustup.rs)" >&2; exit 1; }
command -v npm >/dev/null || { echo "install.sh: needs Node.js and npm to build the popover UI (https://nodejs.org)" >&2; exit 1; }

if ! cargo tauri --version >/dev/null 2>&1; then
  run cargo install tauri-cli --version '^2' --locked
fi

echo "== build"
(cd "$here/app" && run cargo tauri build --bundles app)
built="$root/target/release/bundle/macos/tether.app"

echo "== first sync plan"
if [ "$dry" = 1 ]; then
  echo "+ $built/Contents/MacOS/tether status"
  echo "+ $built/Contents/MacOS/tether sync --dry-run"
else
  "$built/Contents/MacOS/tether" status || true
  if ! "$built/Contents/MacOS/tether" sync --dry-run; then
    echo "install.sh: the dry-run sync failed (see above); fix that before installing" >&2
    exit 1
  fi
fi
if [ "$dry" = 0 ] && [ "$yes" = 0 ]; then
  printf "Install tether and start it now? It applies the plan above and keeps syncing. [y/N] "
  read -r answer
  case "$answer" in
    y|Y|yes) ;;
    *) echo "Nothing installed."; exit 0 ;;
  esac
fi

echo "== install to $app"
if [ "$dry" = 0 ]; then bootout; else echo "+ launchctl bootout $domain/$label (if loaded)"; fi
run mkdir -p "$apps" "$(dirname "$link")"
run rm -rf "$app"
run cp -R "$built" "$app"
run ln -sfn "$bin" "$link"

echo "== login item $agent"
plist="<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
  <key>Label</key><string>$label</string>
  <key>ProgramArguments</key><array><string>$bin</string></array>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Interactive</string>
</dict>
</plist>"
if [ "$dry" = 1 ]; then
  echo "+ write $agent"
else
  mkdir -p "$(dirname "$agent")"
  printf '%s\n' "$plist" > "$agent"
fi
run launchctl bootstrap "$domain" "$agent"
echo "done"
