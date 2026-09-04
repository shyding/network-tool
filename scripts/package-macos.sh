#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CACHE="$ROOT/.build-cache/macos-universal.sha256"
DIST="$ROOT/dist/macos-universal"
FORCE="${FORCE:-0}"

fingerprint() {
  { find "$ROOT/src-tauri/src" "$ROOT/src-tauri/icons" "$ROOT/analysis/tauri-assets" -type f -print
    printf '%s\n' "$ROOT/src-tauri/Cargo.toml" "$ROOT/src-tauri/Cargo.lock" "$ROOT/src-tauri/build.rs" "$ROOT/src-tauri/tauri.conf.json"
  } | LC_ALL=C sort -u | while IFS= read -r file; do shasum -a 256 "$file"; done | shasum -a 256 | awk '{print $1}'
}

[[ "$(uname -s)" == "Darwin" ]] || { echo 'macOS 安装包必须在 macOS 主机上生成。' >&2; exit 2; }
mkdir -p "$(dirname "$CACHE")" "$DIST"
HASH="$(fingerprint)"
OLD="$(test -f "$CACHE" && cat "$CACHE" || true)"
if [[ "$FORCE" != "1" && "$HASH" == "$OLD" ]] && find "$DIST" -type f -print -quit | grep -q .; then
  echo "源码未变化，复用 macOS 产物：$DIST"
  find "$DIST" -maxdepth 1 -type f -print
  exit 0
fi

command -v rustup >/dev/null 2>&1 || { echo '请先安装 rustup。' >&2; exit 2; }
rustup target add aarch64-apple-darwin x86_64-apple-darwin
if ! command -v cargo-tauri >/dev/null 2>&1; then cargo install cargo-tauri --version '^2' --locked; fi

echo '提示：当前后端包含 Windows 专用能力；macOS 平台适配完成后本脚本可直接产出通用 app/DMG。'
(cd "$ROOT/src-tauri" && cargo tauri build --target universal-apple-darwin --bundles app,dmg)
find "$ROOT/src-tauri/target/universal-apple-darwin/release/bundle" -name '*.dmg' -type f -exec cp -f {} "$DIST/" \;
printf '%s' "$HASH" > "$CACHE"
echo "macOS 产物已生成：$DIST"

