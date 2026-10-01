#!/usr/bin/env bash
# Actual toolchain raw profiles exercise validation without Cargo or a model.
set -euo pipefail
export LC_ALL=C
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
validator="$repo/scripts/validate-coverage-profiles.sh"
scratch=$(mktemp -d)
trap 'rm -rf -- "$scratch"' EXIT
host=$(rustc -vV | sed -n 's/^host: //p')
profdata="$(rustc --print sysroot)/lib/rustlib/$host/bin/llvm-profdata"
test -x "$profdata"
printf 'fn main() { assert_eq!(std::hint::black_box(2) + 2, 4); }\n' > "$scratch/main.rs"
rustc -C instrument-coverage "$scratch/main.rs" -o "$scratch/fixture"
mkdir "$scratch/mixed" "$scratch/mixed/nested"
LLVM_PROFILE_FILE="$scratch/mixed/valid.profraw" "$scratch/fixture"
valid_sha=$(sha256sum "$scratch/mixed/valid.profraw" | cut -d ' ' -f 1)
head -c 16 "$scratch/mixed/valid.profraw" > "$scratch/mixed/truncated.profraw"
truncated_sha=$(sha256sum "$scratch/mixed/truncated.profraw" | cut -d ' ' -f 1)
touch "$scratch/mixed/empty.profraw"
cp "$scratch/mixed/truncated.profraw" "$scratch/mixed/nested/untouched.profraw"
bash "$validator" "$scratch/mixed"
test "$(sha256sum "$scratch/mixed/valid.profraw" | cut -d ' ' -f 1)" = "$valid_sha"
test "$(sha256sum "$scratch/mixed/truncated.profraw.invalid" | cut -d ' ' -f 1)" = "$truncated_sha"
test -f "$scratch/mixed/empty.profraw.invalid"
test -s "$scratch/mixed/truncated.profraw.invalid.diagnostics"
test -s "$scratch/mixed/empty.profraw.invalid.diagnostics"
test ! -e "$scratch/mixed/truncated.profraw"
test ! -e "$scratch/mixed/empty.profraw"
test -f "$scratch/mixed/nested/untouched.profraw"
"$profdata" merge -sparse "$scratch/mixed/valid.profraw" -o "$scratch/merged.profdata"
test -s "$scratch/merged.profdata"
mkdir "$scratch/native-mixed"
cp "$scratch/mixed/valid.profraw" "$scratch/native-mixed/ordinary.profraw"
cp "$scratch/mixed/valid.profraw" "$scratch/native-mixed/native-valid.profraw"
cp "$scratch/mixed/truncated.profraw.invalid" "$scratch/native-mixed/native-truncated.profraw"
bash "$validator" "$scratch/native-mixed"
test "$(sha256sum "$scratch/native-mixed/native-valid.profraw" | cut -d ' ' -f 1)" = "$valid_sha"
test "$(sha256sum "$scratch/native-mixed/native-truncated.profraw.invalid" | cut -d ' ' -f 1)" = "$truncated_sha"
test -s "$scratch/native-mixed/native-truncated.profraw.invalid.diagnostics"
test ! -e "$scratch/native-mixed/native-truncated.profraw"

must_fail() {
  if "$@" > "$scratch/failure.stdout" 2> "$scratch/failure.stderr"; then
    printf 'expected validator failure\n' >&2
    exit 1
  fi
}
mkdir "$scratch/all-invalid"
touch "$scratch/all-invalid/empty.profraw"
must_fail bash "$validator" "$scratch/all-invalid"
test -f "$scratch/all-invalid/empty.profraw.invalid"
mkdir "$scratch/native-only"
cp "$scratch/mixed/valid.profraw" "$scratch/native-only/native-valid.profraw"
must_fail bash "$validator" "$scratch/native-only"
test -f "$scratch/native-only/native-valid.profraw"
mkdir "$scratch/tool-failure"
cp "$scratch/mixed/truncated.profraw.invalid" "$scratch/tool-failure/bad.profraw"
must_fail bash "$validator" "$scratch/tool-failure" "$scratch/missing-tool"
printf '#!/usr/bin/env bash\nprintf "error: simulated I/O failure\\n" >&2\nexit 1\n' > "$scratch/fake-tool"
chmod +x "$scratch/fake-tool"
must_fail bash "$validator" "$scratch/tool-failure" "$scratch/fake-tool"
test -f "$scratch/tool-failure/bad.profraw"
test ! -e "$scratch/tool-failure/bad.profraw.invalid"
mkdir "$scratch/version-mismatch"
cp "$scratch/mixed/valid.profraw" "$scratch/version-mismatch/new-version.profraw"
printf '\377\377\377\377\377\377\377\377' | \
  dd of="$scratch/version-mismatch/new-version.profraw" bs=1 seek=8 conv=notrunc status=none
must_fail bash "$validator" "$scratch/version-mismatch"
test -f "$scratch/version-mismatch/new-version.profraw"
test ! -e "$scratch/version-mismatch/new-version.profraw.invalid"

mkdir "$scratch/collision"
touch "$scratch/collision/empty.profraw"
printf 'existing evidence\n' > "$scratch/collision/empty.profraw.invalid"
must_fail bash "$validator" "$scratch/collision"
test -f "$scratch/collision/empty.profraw"
test "$(cat "$scratch/collision/empty.profraw.invalid")" = 'existing evidence'
rm "$scratch/collision/empty.profraw.invalid"
printf 'existing diagnostics\n' > "$scratch/collision/empty.profraw.invalid.diagnostics"
must_fail bash "$validator" "$scratch/collision"
test -f "$scratch/collision/empty.profraw"
test "$(cat "$scratch/collision/empty.profraw.invalid.diagnostics")" = 'existing diagnostics'
mkdir "$scratch/directory-collision"
touch "$scratch/directory-collision/empty.profraw"
mkdir "$scratch/directory-collision/empty.profraw.invalid"
must_fail bash "$validator" "$scratch/directory-collision"
test -f "$scratch/directory-collision/empty.profraw"
test -z "$(find "$scratch/directory-collision/empty.profraw.invalid" -mindepth 1 -print -quit)"

mkdir "$scratch/indirect"
ln -s "$scratch/mixed/valid.profraw" "$scratch/indirect/link.profraw"
must_fail bash "$validator" "$scratch/indirect"
test -L "$scratch/indirect/link.profraw"
printf 'coverage profile validator regression fixtures passed\n'
