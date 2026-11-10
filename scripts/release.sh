#!/usr/bin/env bash
# ============================================================================
# 发布脚本 —— 版本号只在这里递增，CI 只读不写
#
#   ./scripts/release.sh          递增 patch（1.0.1 → 1.0.2）并打 tag
#   ./scripts/release.sh minor    递增 minor（1.0.2 → 1.1.0）
#   ./scripts/release.sh 2.0.0    指定版本
#   ./scripts/release.sh --dry    只看会做什么，不执行
#
# 流程：改版本 → 本地构建验证 → 提交 → 打 tag → 推送
# tag 推送后 GitHub Actions 自动构建多平台产物并发布 Release
# ============================================================================
set -euo pipefail

cd "$(dirname "$0")/.."

DRY=0
BUMP="patch"
for arg in "$@"; do
  case "$arg" in
    --dry) DRY=1 ;;
    major|minor|patch) BUMP="$arg" ;;
    [0-9]*) NEW_VERSION="$arg" ;;
    *) echo "未知参数: $arg" >&2; exit 1 ;;
  esac
done

CURRENT=$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')
MAJOR=${CURRENT%%.*}
REST=${CURRENT#*.}
MINOR=${REST%%.*}
PATCH=${REST#*.}

if [ -z "${NEW_VERSION:-}" ]; then
  case "$BUMP" in
    major) NEW_VERSION="$((MAJOR + 1)).0.0" ;;
    minor) NEW_VERSION="${MAJOR}.$((MINOR + 1)).0" ;;
    patch) NEW_VERSION="${MAJOR}.${MINOR}.$((PATCH + 1))" ;;
  esac
fi

TAG="v${NEW_VERSION}"
echo "版本: ${CURRENT} → ${NEW_VERSION}  (tag ${TAG})"

if [ "$DRY" = "1" ]; then
  echo "[dry] 只预览，不执行"
  exit 0
fi

# ---- 1. 工作区必须干净 ----
if [ -n "$(git status --porcelain)" ]; then
  echo "工作区有未提交改动，先提交或 stash：" >&2
  git status --short >&2
  exit 1
fi

# ---- 2. 本地构建 + 测试，通过才继续 ----
echo "→ 构建与测试…"
cargo build --release --quiet
BUILD_LOG=$(cargo build --release 2>&1 || true)
if echo "$BUILD_LOG" | grep -A2 "^warning:" | grep -qE "^\s+--> src/"; then
  echo "项目源码有警告，先修掉：" >&2
  echo "$BUILD_LOG" | grep -A2 "^warning:" | grep -B2 -E "^\s+--> src/" >&2
  exit 1
fi
cargo test --release --quiet

# ---- 3. 改版本 ----
sed -i "0,/^version = \".*\"/s//version = \"${NEW_VERSION}\"/" Cargo.toml
# Cargo.lock 交给 cargo 自己同步，避免手改破坏结构
cargo update -p memory-kb --precise "$NEW_VERSION" 2>/dev/null || cargo generate-lockfile --quiet

grep -m1 '^version' Cargo.toml
grep -A1 'name = "memory-kb"' Cargo.lock | head -2

# ---- 4. 提交 + 打 tag ----
git add Cargo.toml Cargo.lock
git commit -q -m "chore(release): ${TAG}"
git tag -a "$TAG" -m "Release ${TAG}"

echo ""
echo "完成。接下来执行："
echo "  git push origin main --follow-tags"
echo ""
echo "推送后 GitHub Actions 会自动："
echo "  · 构建 linux-amd64（静态）+ windows-amd64"
echo "  · 创建 Release ${TAG}"
