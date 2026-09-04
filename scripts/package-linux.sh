#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CACHE="$ROOT/.build-cache/linux-x64.sha256"
DIST="$ROOT/dist/linux-x64"
FORCE="${FORCE:-0}"

fingerprint() {
  { find "$ROOT/src-tauri/src" "$ROOT/src-tauri/icons" "$ROOT/analysis/tauri-assets" -type f -print
    printf '%s\n' "$ROOT/src-tauri/Cargo.toml" "$ROOT/src-tauri/Cargo.lock" "$ROOT/src-tauri/build.rs" "$ROOT/src-tauri/tauri.conf.json"
  } | LC_ALL=C sort -u | while IFS= read -r file; do
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$file"; else shasum -a 256 "$file"; fi
  done | { if command -v sha256sum >/dev/null 2>&1; then sha256sum; else shasum -a 256; fi; } | awk '{print $1}'
}

mkdir -p "$(dirname "$CACHE")" "$DIST"
HASH="$(fingerprint)"
OLD="$(test -f "$CACHE" && cat "$CACHE" || true)"
if [[ "$FORCE" != "1" && "$HASH" == "$OLD" ]] && find "$DIST" -type f -print -quit | grep -q .; then
  echo "源码未变化，复用 Linux 产物：$DIST"
  find "$DIST" -maxdepth 1 -type f -print
  exit 0
fi

if ! command -v cargo-tauri >/dev/null 2>&1; then
  echo '首次使用：安装 cargo-tauri（以后将直接复用）...'
  cargo install cargo-tauri --version '^2' --locked
fi

echo '提示：当前后端包含 Windows 专用能力；Linux 平台适配完成后本脚本可直接产出 deb/AppImage。'
(cd "$ROOT/src-tauri" && cargo tauri build --bundles deb,appimage)
find "$ROOT/src-tauri/target/release/bundle" \( -name '*.deb' -o -name '*.AppImage' \) -type f -exec cp -f {} "$DIST/" \;
printf '%s' "$HASH" > "$CACHE"
echo "Linux 产物已生成：$DIST"

