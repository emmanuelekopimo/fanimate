#!/usr/bin/env bash
# Fetch Rhubarb Lip Sync, which Fanimate runs for lip sync, into <out>.
#
#   scripts/fetch-rhubarb.sh <windows|linux|macos> <out>
#
# Rhubarb is MIT licensed (its LICENSE.md, kept beside it, also lists the
# libraries it includes). Only the program, its speech models (res) and the
# licence are kept. The macOS build is for Intel Macs and runs on Apple
# silicon through Rosetta 2.
set -euo pipefail

VERSION=1.14.0
case "$1" in
  windows) platform=Windows sha=62fa416a8d5e382a3828ee4bef358ce520d0b4cabdeaea75a7ac266d098d1fe3 ;;
  linux) platform=Linux sha=a9a9074862cff47b2d59b8bf399a678a3b0b74f9452ad6ad94cb292913dd8667 ;;
  macos) platform=macOS sha=f991deacac6c973a14a4431a16a58b842f436531e120cfaea142c87c0d3ab4c5 ;;
  *) echo "usage: $0 <windows|linux|macos> <out>" >&2; exit 2 ;;
esac

out=$(mkdir -p "$2" && cd "$2" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
name="Rhubarb-Lip-Sync-$VERSION-$platform"
curl -sSfL -o "$work/$name.zip" \
  "https://github.com/DanielSWolf/rhubarb-lip-sync/releases/download/v$VERSION/$name.zip"
if command -v sha256sum > /dev/null; then
  echo "$sha  $work/$name.zip" | sha256sum -c -
else
  echo "$sha  $work/$name.zip" | shasum -a 256 -c -
fi
if command -v unzip > /dev/null; then
  unzip -q "$work/$name.zip" -d "$work"
else
  7z x -y -o"$work" "$work/$name.zip" > /dev/null
fi
for item in rhubarb rhubarb.exe res LICENSE.md; do
  if [ -e "$work/$name/$item" ]; then
    cp -R "$work/$name/$item" "$out/"
  fi
done
echo "== Rhubarb Lip Sync $VERSION in $out"
