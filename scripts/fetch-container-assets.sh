#!/bin/sh
# Build-time resources only; the running service never downloads these files.
set -eu
manifest=${1:?manifest required}
output=${2:?output directory required}
target_arch=${3:?target architecture required}
mkdir -p "$output"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM

download() {
    curl --fail --location --retry 5 --retry-all-errors --connect-timeout 30 \
        --max-time 600 --output "$2" "$1"
}

runtime_url=$(jq -er --arg arch "$target_arch" '.native_runtime.archives[$arch].url' "$manifest")
runtime_hash=$(jq -er --arg arch "$target_arch" '.native_runtime.archives[$arch].sha256' "$manifest")
download "$runtime_url" "$scratch/runtime.tgz"
printf '%s  %s\n' "$runtime_hash" "$scratch/runtime.tgz" | sha256sum --check --strict
mkdir -p "$scratch/runtime" "$output/onnxruntime"
tar -xzf "$scratch/runtime.tgz" --strip-components=1 -C "$scratch/runtime"
cp -a "$scratch/runtime/lib" "$output/onnxruntime/"
cp "$scratch/runtime/LICENSE" "$scratch/runtime/ThirdPartyNotices.txt" "$output/onnxruntime/"

jq -r '.assets[] | [.directory, .name, .url, .sha3_256] | @tsv' "$manifest" |
while IFS="$(printf '\t')" read -r directory name url expected; do
    mkdir -p "$output/$directory"
    destination="$output/$directory/$name"
    download "$url" "$destination"
    actual=$(openssl dgst -sha3-256 -r "$destination" | cut -d ' ' -f 1)
    if [ "$actual" != "$expected" ]; then
        printf 'SHA3-256 mismatch: %s\n' "$name" >&2
        exit 1
    fi
done

mkdir -p "$output/licenses"
jq -r '.licenses[] | [.name, .url, .sha256] | @tsv' "$manifest" |
while IFS="$(printf '\t')" read -r name url expected; do
    destination="$output/licenses/$name"
    download "$url" "$destination"
    printf '%s  %s\n' "$expected" "$destination" | sha256sum --check --strict
done
cp "$manifest" "$output/manifest.json"
