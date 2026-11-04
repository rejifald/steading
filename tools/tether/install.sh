#!/bin/sh
# Builds tether and installs it on this Mac (ADR 0001 rule 12). The steps are
# scripts/install-app.sh's; tether's first plan is `sync --dry-run`.
#
#   ./install.sh              # build, plan, confirm, install, start
#   ./install.sh --dry-run    # print the steps, change nothing
#   ./install.sh --yes        # no confirmation prompt
#   ./install.sh --uninstall  # remove the login item, link, app
#
# It runs as you, with your git credentials. --uninstall leaves
# ~/.config/tether (repos.toml, backups) and every listed clone alone.
set -eu
here=$(cd "$(dirname "$0")" && pwd -P)
exec "$here/../../scripts/install-app.sh" tether "sync --dry-run" \
  "applies the plan above and keeps syncing" "$@"
