#!/bin/sh
# Builds reap and installs it on this Mac (ADR 0001 rule 16). The steps are
# scripts/install-app.sh's; reap's first plan is `sweep --dry-run`.
#
#   ./install.sh              # build, plan, confirm, install, start
#   ./install.sh --dry-run    # print the steps, change nothing
#   ./install.sh --yes        # no confirmation prompt
#   ./install.sh --uninstall  # remove the login item, link, app
#
# It runs as you, with your git and gh credentials. --uninstall leaves
# ~/.config/reap (repos.toml, state) and every listed clone alone.
set -eu
here=$(cd "$(dirname "$0")" && pwd -P)
exec "$here/../../scripts/install-app.sh" reap "sweep --dry-run" \
  "removes merged worktrees once their grace period passes" "$@"
