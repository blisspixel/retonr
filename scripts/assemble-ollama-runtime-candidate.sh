#!/usr/bin/env bash

set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "usage: $0 <ollama-linux-amd64.tar.zst> <retonr-isolation> <output-directory>" >&2
  exit 2
fi

archive="$1"
helper="$2"
output="$3"
script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repository_root="$(cd "$script_directory/.." && pwd)"
review_directory="$repository_root/docs/reviews/runtime-packages/ollama-v0.32.15-linux-x86_64-gnu"
transformation="$review_directory/transformation.json"

for command_name in jq sha256sum stat tar; do
  command -v "$command_name" >/dev/null || {
    echo "required command is unavailable: $command_name" >&2
    exit 2
  }
done

test -f "$archive" || {
  echo "source archive is unavailable" >&2
  exit 2
}
test -f "$helper" || {
  echo "Retonr isolation helper is unavailable" >&2
  exit 2
}
test -f "$transformation" || {
  echo "checked-in transformation evidence is unavailable" >&2
  exit 2
}
test ! -e "$output" || {
  echo "output path already exists" >&2
  exit 2
}

expected_archive_size=1422416084
expected_archive_digest=50539c5fe9bf85887733355098dcdb266b433cb8c73fa180713417e9ed6e42bb
actual_archive_size="$(stat -c '%s' "$archive")"
actual_archive_digest="$(sha256sum "$archive" | cut -d' ' -f1)"
test "$actual_archive_size" = "$expected_archive_size" || {
  echo "source archive size does not match the frozen review" >&2
  exit 1
}
test "$actual_archive_digest" = "$expected_archive_digest" || {
  echo "source archive digest does not match the frozen review" >&2
  exit 1
}

work_directory="$(mktemp -d)"
cleanup() {
  rm -rf -- "$work_directory"
}
trap cleanup EXIT

actual_tree="$work_directory/actual-tree.txt"
expected_tree="$work_directory/expected-tree.txt"
tar --zstd -tf "$archive" |
  sed -e '/\/$/d' |
  LC_ALL=C sort >"$actual_tree"
jq -r '
  .selected_source_entries[].source_path,
  .consumed_symlinks[],
  .excluded_source_entries[]
' "$transformation" | LC_ALL=C sort >"$expected_tree"
test "$(wc -l <"$actual_tree")" -eq 51
test "$(wc -l <"$expected_tree")" -eq 51
diff -u "$expected_tree" "$actual_tree"

mapfile -t selected_sources < <(
  jq -r '.selected_source_entries[].source_path' "$transformation"
)
mapfile -t consumed_symlinks < <(jq -r '.consumed_symlinks[]' "$transformation")
tar --zstd -xf "$archive" -C "$work_directory" -- \
  "${selected_sources[@]}" "${consumed_symlinks[@]}"

declare -A expected_links=(
  [lib/ollama/libggml-base.so.0]=libggml-base.so.0.20.2
  [lib/ollama/libggml.so.0]=libggml.so.0.20.2
  [lib/ollama/libllama-common.so.0]=libllama-common.so.0.1.2
  [lib/ollama/libllama.so.0]=libllama.so.0.1.2
  [lib/ollama/libmtmd.so.0]=libmtmd.so.0.1.2
)
for link_path in "${!expected_links[@]}"; do
  test -L "$work_directory/$link_path"
  test "$(readlink "$work_directory/$link_path")" = "${expected_links[$link_path]}"
done

mkdir -p "$output"
while IFS=$'\t' read -r source_path output_path byte_size digest; do
  source_file="$work_directory/$source_path"
  destination="$output/$output_path"
  test -f "$source_file"
  test "$(stat -c '%s' "$source_file")" = "$byte_size"
  test "$(sha256sum "$source_file" | cut -d' ' -f1)" = "$digest"
  mkdir -p "$(dirname "$destination")"
  install -m 0755 "$source_file" "$destination"
done < <(
  jq -r '
    .selected_source_entries[] |
    [.source_path, .output_path, (.byte_size | tostring), .sha256] |
    @tsv
  ' "$transformation"
)

helper_path="$(jq -r '.added_member.output_path' "$transformation")"
helper_size="$(jq -r '.added_member.byte_size' "$transformation")"
helper_digest="$(jq -r '.added_member.sha256' "$transformation")"
test "$(stat -c '%s' "$helper")" = "$helper_size"
test "$(sha256sum "$helper" | cut -d' ' -f1)" = "$helper_digest"
mkdir -p "$output/$(dirname "$helper_path")"
install -m 0755 "$helper" "$output/$helper_path"

actual_output="$work_directory/actual-output.txt"
expected_output="$work_directory/expected-output.txt"
find "$output" -type f -printf '%P\n' | LC_ALL=C sort >"$actual_output"
{
  jq -r '.selected_source_entries[].output_path' "$transformation"
  printf '%s\n' "$helper_path"
} | LC_ALL=C sort >"$expected_output"
diff -u "$expected_output" "$actual_output"

while IFS=$'\t' read -r relative_path byte_size digest; do
  member="$output/$relative_path"
  test "$(stat -c '%s' "$member")" = "$byte_size"
  test "$(sha256sum "$member" | cut -d' ' -f1)" = "$digest"
done < <(
  {
    jq -r '
      .selected_source_entries[] |
      [.output_path, (.byte_size | tostring), .sha256] |
      @tsv
    ' "$transformation"
    printf '%s\t%s\t%s\n' "$helper_path" "$helper_size" "$helper_digest"
  } | LC_ALL=C sort
)

echo "assembled exact non-admitted Ollama runtime candidate at $output"
