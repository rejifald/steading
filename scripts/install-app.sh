#!/bin/sh
# Builds one of steading's menu-bar apps and installs it on this Mac. Each tool's
# install.sh calls this with its name and the read-only plan to show first:
#   1. cargo tauri build  -> <tool>.app (unsigned; a local build needs none), which
#      builds the popover UI with npm first
#   2. show `git <tool> status` and the plan command, and ask to confirm; on "no"
#      nothing is installed, and a failing plan stops the install
#   3. copy to ~/Applications, link the CLI to ~/.local/bin/git-<tool>
#   4. register the login item (a LaunchAgent) and start it
#
#   install-app.sh <tool> <plan command> <what it does> [--dry-run] [--yes] [--uninstall]
#
#   --dry-run    print the steps, change nothing
#   --yes        no confirmation prompt
#   --uninstall  remove the login item, link, app
#
# It runs as you, with your git credentials. --uninstall leaves ~/.config/<tool>
# and every listed clone alone.
set -eu

[ $# -ge 3 ] || { echo "usage: install-app.sh <tool> <plan command> <what it does> [options]" >&2; exit 2; }
tool=$1 plan=$2 what=$3
shift 3

dry=0 yes=0 uninstall=0
for a in "$@"; do
  case "$a" in
    --dry-run) dry=1 ;;
    --yes|-y) yes=1 ;;
    --uninstall) uninstall=1 ;;
    *) echo "install.sh: unknown option $a" >&2; exit 2 ;;
  esac
done

root=$(cd "$(dirname "$0")/.." && pwd -P)
app_dir="$root/tools/$tool/app"
[ -d "$app_dir" ] || { echo "install-app.sh: no app at tools/$tool/app" >&2; exit 2; }
label="io.github.rejifald.$tool"
apps="$HOME/Applications"
app="$apps/$tool.app"
bin="$app/Contents/MacOS/$tool"
link="$HOME/.local/bin/git-$tool"
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
    echo "left alone: $link is not git-$tool's link"
  fi
  echo "== app $app"
  if [ -e "$app" ]; then run rm -rf "$app"; fi
  echo "done; ~/.config/$tool and the listed clones are untouched"
  exit 0
fi

command -v cargo >/dev/null || { echo "install.sh: needs a Rust toolchain (https://rustup.rs)" >&2; exit 1; }
command -v npm >/dev/null || { echo "install.sh: needs Node.js and npm to build the popover UI (https://nodejs.org)" >&2; exit 1; }

if ! cargo tauri --version >/dev/null 2>&1; then
  run cargo install tauri-cli --version '^2' --locked
fi

echo "== build"
(cd "$app_dir" && run cargo tauri build --bundles app)
built="$root/target/release/bundle/macos/$tool.app"
built_bin="$built/Contents/MacOS/$tool"

echo "== first plan"
if [ "$dry" = 1 ]; then
  echo "+ $built_bin status"
  echo "+ $built_bin $plan"
else
  "$built_bin" status || true
  # $plan is a fixed subcommand from the tool's own install.sh; split it into words.
  # shellcheck disable=SC2086
  if ! "$built_bin" $plan; then
    echo "install.sh: '$plan' failed (see above); fix that before installing" >&2
    exit 1
  fi
fi
if [ "$dry" = 0 ] && [ "$yes" = 0 ]; then
  printf "Install %s and start it now? It %s. [y/N] " "$tool" "$what"
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
