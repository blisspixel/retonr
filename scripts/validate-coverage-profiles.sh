#!/usr/bin/env bash
# Validate only completed root-level raw profiles without weakening coverage gates.
set -euo pipefail
export LC_ALL=C

if [[ $# -lt 1 || $# -gt 2 ]]; then
  printf 'usage: %s PROFILE_DIRECTORY [LLVM_PROFDATA]\n' "$0" >&2
  exit 2
fi
profile_dir=$1
test -d "$profile_dir" && test ! -L "$profile_dir"
host=$(rustc -vV | sed -n 's/^host: //p')
expected_tool="$(rustc --print sysroot)/lib/rustlib/$host/bin/llvm-profdata"
profdata=${2:-$expected_tool}
test -x "$profdata"
test "$(realpath "$profdata")" = "$(realpath "$expected_tool")"
"$profdata" --version >/dev/null

scratch=$(mktemp -d)
trap 'rm -rf -- "$scratch"' EXIT
valid=0
ordinary=0
invalid=0
# Materialize discovery so find failures cannot be hidden in process substitution.
find "$profile_dir" -mindepth 1 -maxdepth 1 -name '*.profraw' -print0 > "$scratch/profiles"
while IFS= read -r -d '' profile; do
  if [[ ! -f "$profile" || -L "$profile" || ! -r "$profile" ]]; then
    printf 'coverage profile must be a readable regular file: %s\n' "$profile" >&2
    exit 1
  fi
  if [[ ! -s "$profile" ]]; then
    # LLVM can accept zero-byte input as an empty indexed profile. It contains
    # no execution evidence, so retain it under the same quarantine contract.
    : > "$scratch/stdout"
    printf 'warning: %s: empty raw profile file\n' "$profile" > "$scratch/stderr"
  elif "$profdata" merge -sparse "$profile" -o "$scratch/check.profdata" \
      > "$scratch/stdout" 2> "$scratch/stderr"; then
    # Successful merges with warnings also require investigation, not silent loss.
    test ! -s "$scratch/stderr"
    valid=$((valid + 1))
    if [[ $(basename "$profile") != native-* ]]; then
      ordinary=$((ordinary + 1))
    fi
    continue
  fi
  # Recognize only the pinned LLVM diagnostics for malformed raw bytes. A version
  # mismatch, read failure, unsupported tool or unexpected diagnostic is fatal.
  recognized=0
  unexpected=0
  while IFS= read -r line || [[ -n "$line" ]]; do
    case "$line" in
      "warning: $profile: invalid instrumentation profile data (file header is corrupt)" | \
      "warning: $profile: invalid instrumentation profile data (truncated profile data)" | \
      "warning: $profile: empty raw profile file") recognized=$((recognized + 1)) ;;
      'error: no profile can be merged') ;;
      *) unexpected=1 ;;
    esac
  done < "$scratch/stderr"
  if [[ $recognized -ne 1 || $unexpected -ne 0 || -s "$scratch/stdout" ]]; then
    cat "$scratch/stderr" >&2
    exit 1
  fi
  quarantine="$profile.invalid"
  diagnostics="$quarantine.diagnostics"
  # Hard-link creation is exclusive and preserves the exact invalid bytes. No
  # existing destination or symlink can be overwritten. Unlink only after both
  # retained artifacts exist; any failure leaves the original raw file intact.
  ln -T -- "$profile" "$quarantine"
  if ! (set -o noclobber; cat "$scratch/stderr" > "$diagnostics"); then
    printf 'cannot retain coverage profile diagnostics: %s\n' "$diagnostics" >&2
    exit 1
  fi
  rm -- "$profile"
  invalid=$((invalid + 1))
done < "$scratch/profiles"
printf 'root coverage profiles: %s valid (%s ordinary), %s quarantined\n' "$valid" "$ordinary" "$invalid"
test "$ordinary" -gt 0
