#!/usr/bin/env bash
# Refuses sensitive information in what is about to be published (CLAUDE.md § Commits):
# local paths, secrets, email addresses, and an author identity that is not a GitHub
# noreply address.
#
#   scripts/check-sensitive.sh --staged          # added lines of the staged diff, and the identity
#   scripts/check-sensitive.sh --message <file>  # a commit message (the commit-msg hook)
#   scripts/check-sensitive.sh --all             # every tracked file
#
# Prints `kind: where` per hit (a secret's value is never echoed) and exits 1 on any hit.
set -euo pipefail

grep_e() { /usr/bin/grep -E "$@" || true; }

self="scripts/check-sensitive.sh"

# Absolute home and temp directories. `/Users/me` and `/home/me` are the test placeholder.
local_path='(/Users/|/home/)[A-Za-z0-9._-]+|/private/(tmp|var)/|/var/folders/|[A-Za-z]:\\Users\\'
local_path_ok='(/Users/|/home/)me([^A-Za-z0-9._-]|$)'
secret='gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|xox[abpr]-[A-Za-z0-9-]{10,}|-----BEGIN [A-Z ]*PRIVATE KEY-----|(api[_-]?key|secret|token|password)["'\'']?[[:space:]]*[:=][[:space:]]*["'\''][^"'\'' ]{8,}|://[^/[:space:]:@]+:[^/[:space:]@]+@'
email='[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}'
email_ok='@users\.noreply\.github\.com|^noreply@anthropic\.com$|@example\.(com|org|net|invalid)$|\.invalid$|^git@github\.com$|@[0-9]+x\.[a-z]+$'
noreply='@users\.noreply\.github\.com$'

hits=0
report() {
  echo "check-sensitive: $1: $2" >&2
  hits=$((hits + 1))
}

# Reads `where<TAB>text` lines on stdin and reports every hit. Each pattern runs once
# over the whole stream; only the few lines it matches are looked at one by one, so a
# lockfile's hundred thousand lines cost three greps, not a process per line.
scan() {
  local stream where text match
  stream=$(grep_e -v "^$self:")
  while IFS=$'\t' read -r where text; do
    while IFS= read -r match; do
      [ -n "$match" ] && report "local path ($match)" "$where"
    done < <(printf '%s\n' "$text" | grep_e -o "$local_path" | grep_e -v "$local_path_ok")
  done < <(printf '%s\n' "$stream" | grep_e $'\t'".*($local_path)")
  while IFS=$'\t' read -r where text; do
    [ -n "$where" ] && report "possible secret" "$where"
  done < <(printf '%s\n' "$stream" | grep_e -i $'\t'".*($secret)")
  while IFS=$'\t' read -r where text; do
    while IFS= read -r match; do
      [ -n "$match" ] && report "email address ($match)" "$where"
    done < <(printf '%s\n' "$text" | grep_e -o "$email" | grep_e -v "$email_ok")
  done < <(printf '%s\n' "$stream" | grep_e $'\t'".*$email")
}

# The staged diff's added lines as `file:line<TAB>text`, plus each staged path itself.
staged_lines() {
  git diff --cached --no-color --no-ext-diff --diff-filter=ACMR -U0 | awk '
    /^\+\+\+ b\// { file = substr($0, 7); print file ":name\t" file; next }
    /^@@/ { split($3, a, ","); line = substr(a[1], 2) + 0; next }
    /^\+/ { print file ":" line "\t" substr($0, 2); line++ }
  '
}

tracked_lines() {
  git ls-files -z | while IFS= read -r -d '' f; do
    printf '%s:name\t%s\n' "$f" "$f"
    # Binary files (icons) carry no text worth scanning.
    if [ -f "$f" ] && ! /usr/bin/grep -Iq . "$f" 2>/dev/null; then continue; fi
    [ -f "$f" ] && awk -v f="$f" '{ print f ":" FNR "\t" $0 }' "$f"
  done
}

check_identity() {
  local who
  for who in "$(git config user.email || true)" "${GIT_AUTHOR_EMAIL:-}" "${GIT_COMMITTER_EMAIL:-}"; do
    [ -z "$who" ] && continue
    if [ -z "$(printf '%s\n' "$who" | grep_e "$noreply")" ]; then
      report "identity is not a GitHub noreply address" "set it with: git config user.email <id>+<login>@users.noreply.github.com"
      return
    fi
  done
}

case "${1:-}" in
  --staged)
    check_identity
    scan < <(staged_lines)
    ;;
  --message)
    [ -f "${2:-}" ] || { echo "check-sensitive: --message needs a file" >&2; exit 2; }
    # Lines git strips from the message (comments) are not published.
    scan < <(awk '!/^#/ { print "commit message:" NR "\t" $0 }' "$2")
    ;;
  --all)
    scan < <(tracked_lines)
    ;;
  *)
    echo "usage: $0 --staged | --message <file> | --all" >&2
    exit 2
    ;;
esac

if [ "$hits" -gt 0 ]; then
  echo "check-sensitive: $hits hit(s); rewrite them (CLAUDE.md § Commits), never --no-verify" >&2
  exit 1
fi
