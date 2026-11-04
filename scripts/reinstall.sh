#!/bin/sh
# Rebuilds and reinstalls every steading app installed on this Mac, from this
# clone. It is meant as git-tether's `after` for the steading clone, so a pull
# that moves `main` updates the apps (README § Updates):
#
#   git tether add ~/Development/steading --branch main --after scripts/reinstall.sh
#
#   scripts/reinstall.sh               # start in the background and return at once
#   scripts/reinstall.sh --foreground  # run here and wait
#
# The background run is a new session, outside tether's process group, because
# reinstalling tether stops tether: run as tether's child it would be killed
# halfway. Its output goes to ~/Library/Logs/steading/reinstall.log, and a
# failure posts a notification, since tether has already seen the hook succeed.
#
# It builds under the clone's tether.lock, so a sync cannot move the tree in the
# middle of a build, and only reinstalls a tool whose app is in ~/Applications.
set -eu

self=$(cd "$(dirname "$0")" && pwd -P)/$(basename "$0")
root=$(cd "$(dirname "$self")/.." && pwd -P)
log_dir="$HOME/Library/Logs/steading"
log="$log_dir/reinstall.log"
# Reap first: reinstalling tether last means a failure there leaves reap updated.
tools="reap tether"

detach() {
  mkdir -p "$log_dir"
  # Fork, leave the caller's session, then run the foreground mode.
  perl -MPOSIX -e 'exit 0 if fork; POSIX::setsid() or die "setsid: $!"; exec @ARGV or die "exec: $!"' \
    "$self" --foreground </dev/null >>"$log" 2>&1
  echo "reinstall started; log: ~/Library/Logs/steading/reinstall.log"
}

notify_failure() {
  osascript -e "display notification \"Reinstall failed for:$1. See ~/Library/Logs/steading/reinstall.log\" with title \"steading\"" || true
}

# launchd gives tether a PATH without Homebrew, ~/.cargo/bin or nvm. Take the PATH
# of an interactive login shell instead: that is where people set those up
# (nvm in .zshrc, which a non-interactive shell does not read).
with_toolchain() {
  if ! command -v cargo >/dev/null || ! command -v npm >/dev/null; then
    # The single quotes are deliberate: $PATH is the login shell's, not ours.
    # shellcheck disable=SC2016
    shell_path=$("${SHELL:-/bin/zsh}" -ilc 'printf "\n%s" "$PATH"' </dev/null 2>/dev/null | tail -n 1) || true
    PATH="${shell_path:+$shell_path:}$PATH"
    export PATH
  fi
  if ! command -v cargo >/dev/null || ! command -v npm >/dev/null; then
    echo "reinstall: cargo and npm are not on your shell's PATH" >&2
    notify_failure " every tool"
    exit 1
  fi
  locked
}

# Hold tether.lock (an flock, ADR 0004 rule 5) for the whole build. $^F keeps the
# descriptor open across exec, so the lock lives as long as the build does.
locked() {
  common_dir=$(git -C "$root" rev-parse --path-format=absolute --git-common-dir)
  exec perl -MFcntl=:flock -e '$^F = 1024; open(my $f, ">>", shift) or die "open: $!"; flock($f, LOCK_EX) or die "flock: $!"; exec @ARGV or die "exec: $!"' \
    "$common_dir/tether.lock" "$self" --locked
}

install_all() {
  echo "== $(date '+%Y-%m-%d %H:%M:%S') reinstall at $(git -C "$root" rev-parse --short HEAD)"
  failed=""
  for tool in $tools; do
    if [ ! -d "$HOME/Applications/$tool.app" ]; then
      echo "== $tool: not installed, skipped"
      continue
    fi
    echo "== $tool"
    "$root/tools/$tool/install.sh" --yes || failed="$failed $tool"
  done
  if [ -n "$failed" ]; then
    echo "== failed:$failed"
    notify_failure "$failed"
    exit 1
  fi
  echo "== done"
}

case "${1:-}" in
  "") detach ;;
  --foreground) with_toolchain ;;
  --locked) install_all ;;
  *) echo "reinstall.sh: unknown option $1" >&2; exit 2 ;;
esac
