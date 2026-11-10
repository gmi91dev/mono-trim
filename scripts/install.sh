#!/usr/bin/env sh
# 安装 / 升级 mtrim（macOS / Linux）
#
# 支持 sh / bash / zsh：
#   sh install.sh [--version 0.0.1] [--dir ~/.local/bin] [--force] [--dry-run]
#
# 环境变量：
#   MONO_TRIM_MIRROR  替换下载源（默认 https://github.com/gmi91dev/mono-trim）

set -eu
# pipefail 并非 POSIX（dash 无此选项），有则启用
if (set -o pipefail) 2>/dev/null; then
    set -o pipefail
fi

VERSION="latest"
DIR="${HOME}/.local/bin"
FORCE=0
DRY_RUN=0

usage() {
    sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
}

while [ $# -gt 0 ]; do
    case "$1" in
        --version) VERSION="${2:?--version 需要参数}"; shift 2 ;;
        --dir)     DIR="${2:?--dir 需要参数}"; shift 2 ;;
        --force)   FORCE=1; shift ;;
        --dry-run) DRY_RUN=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "❌ 未知参数：$1" >&2; usage; exit 1 ;;
    esac
done

REPO="${MONO_TRIM_MIRROR:-https://github.com/gmi91dev/mono-trim}"
REPO="${REPO%/}"

case "$(uname -s)" in
    Darwin) OS="macos" ;;
    Linux)  OS="linux" ;;
    *) echo "❌ 不支持的平台：$(uname -s)（Windows 请使用 scripts/install.ps1 或 scripts/install.cmd）" >&2; exit 1 ;;
esac

case "$(uname -m)" in
    x86_64|amd64) ARCH="x86_64" ;;
    arm64|aarch64) ARCH="aarch64" ;;
    *) echo "❌ 不支持的架构：$(uname -m)" >&2; exit 1 ;;
esac

ASSET="mtrim-${OS}-${ARCH}"
if [ "$VERSION" = "latest" ]; then
    BASE="${REPO}/releases/latest/download"
else
    BASE="${REPO}/releases/download/v${VERSION#v}"
fi

TARGET="${DIR}/mtrim"

echo "→ 下载地址: ${BASE}/${ASSET}"
echo "→ 安装位置: ${TARGET}"

if [ "$DRY_RUN" -eq 1 ]; then
    echo "（DryRun：未下载、未写入）"
    exit 0
fi

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d' ' -f1
    else
        echo "❌ 找不到 sha256sum / shasum，无法校验" >&2
        return 1
    fi
}

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' 0

echo "→ 下载中..."
curl -fsSL --retry 2 -o "${TMP}/${ASSET}" "${BASE}/${ASSET}"
curl -fsSL --retry 2 -o "${TMP}/sha256sums.txt" "${BASE}/sha256sums.txt"

# 兼容 `hash  file`、`hash *file` 与 `hash ./file` 三种格式
EXPECTED="$(awk -v name="$ASSET" '{ f=$NF; gsub(/^(\*|\.\/)/, "", f); if (f == name) { print $1; exit } }' "${TMP}/sha256sums.txt")"
if [ -z "$EXPECTED" ]; then
    echo "❌ 校验文件中没有 ${ASSET} 的记录" >&2
    exit 1
fi
ACTUAL="$(sha256_of "${TMP}/${ASSET}")"
if [ "$EXPECTED" != "$ACTUAL" ]; then
    echo "❌ sha256 校验失败（期望 $EXPECTED，实际 $ACTUAL）" >&2
    exit 1
fi
echo "→ sha256 校验通过"

chmod +x "${TMP}/${ASSET}"
NEW_VERSION="$("${TMP}/${ASSET}" --version | awk '{print $2}')"

OLD_VERSION=""
if [ -x "$TARGET" ]; then
    OLD_VERSION="$("$TARGET" --version 2>/dev/null | awk '{print $2}' || true)"
fi

if [ "$FORCE" -eq 0 ] && [ -n "$OLD_VERSION" ] && [ "$OLD_VERSION" = "$NEW_VERSION" ]; then
    echo "✅ 已是最新（${NEW_VERSION}），无需安装"
    exit 0
fi

mkdir -p "$DIR"
mv -f "${TMP}/${ASSET}" "$TARGET"
chmod +x "$TARGET"

if [ -n "$OLD_VERSION" ]; then
    echo "✅ 已升级：${OLD_VERSION} → ${NEW_VERSION}"
else
    echo "✅ 已安装 mtrim ${NEW_VERSION}"
fi

case ":${PATH}:" in
    *":${DIR}:"*) ;;
    *) echo "ℹ️ 请把 ${DIR} 加入 PATH，例如在 ~/.bashrc 或 ~/.zshrc 中添加："
       echo "     export PATH=\"${DIR}:\$PATH\"" ;;
esac

echo "👉 运行：mtrim --version"
