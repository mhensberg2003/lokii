#!/usr/bin/env bash
# Downloads the prebuilt libmpv that Lokii links and bundles into src-tauri/libmpv/<os>/.
# Usage: scripts/fetch-libmpv.sh [macos-arm64 | macos-x64 | windows-x64]   (default: this machine)
# The archives are copies of upstream builds; see their SOURCES.txt and the libmpv-v1 release.
set -euo pipefail

RELEASE="https://github.com/mhensberg2003/lokii/releases/download/libmpv-v1"

default_platform() {
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) echo macos-arm64 ;;
    Darwin-x86_64) echo macos-x64 ;;
    MINGW* | MSYS* | CYGWIN*) echo windows-x64 ;;
    *) echo "Lokii builds only on macOS and Windows." >&2; exit 1 ;;
  esac
}

platform="${1:-$(default_platform)}"
case "$platform" in
  macos-arm64) sha=ccd92d498de03857575d222c8fc8e039f97a16e0b3bfb00d245382a9c0648963 ;;
  macos-x64) sha=6f2f17fb9fc701a9c2c3fdb2e6ac8f4a157c8d0037ea46126b961ff4d0fca5da ;;
  windows-x64) sha=d549e253b3ccb08443be99ee36da7991306b728990e4df481e266339aa36d1fe ;;
  *) echo "Unknown platform: $platform" >&2; exit 1 ;;
esac

root="$(cd "$(dirname "$0")/.." && pwd)"
dest="$root/src-tauri/libmpv/${platform%%-*}"
archive="$(mktemp -d)/libmpv-$platform.tar.gz"

curl -fsSL -o "$archive" "$RELEASE/libmpv-$platform.tar.gz"
actual="$( (sha256sum "$archive" 2>/dev/null || shasum -a 256 "$archive") | cut -d' ' -f1)"
if [ "$actual" != "$sha" ]; then
  echo "Checksum mismatch for libmpv-$platform.tar.gz: $actual" >&2
  exit 1
fi

rm -rf "$dest"
mkdir -p "$dest"
tar -xzf "$archive" -C "$dest"
echo "libmpv for $platform is in ${dest#"$root"/}"
