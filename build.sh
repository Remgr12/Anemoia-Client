#!/usr/bin/env bash
set -e

TOML="Cargo.toml"
STAMP=".last_build_tree"
INJECT_BIN="target/release/anemoia-inject"

# Read current version from workspace Cargo.toml
current=$(grep '^version' "$TOML" | head -1 | cut -d'"' -f2)

# Hash tracked source files (exclude Cargo.toml itself and the stamp)
tree_hash=$(git ls-files | grep -v '^Cargo\.toml$' | git hash-object --stdin-paths | sha256sum | cut -c1-16)

last_hash=""
[ -f "$STAMP" ] && last_hash=$(cat "$STAMP")

if [ "$tree_hash" != "$last_hash" ]; then
    IFS='.' read -ra parts <<< "$current"
    parts[-1]=$(( ${parts[-1]} + 1 ))
    new_version=$(IFS='.'; echo "${parts[*]}")

    sed -i "s/^version = \"$current\"/version = \"$new_version\"/" "$TOML"
    echo "$tree_hash" > "$STAMP"
    echo "Version bumped: $current → $new_version"
else
    echo "No source changes, keeping version $current"
fi

cargo build --release "$@"
