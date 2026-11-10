#!/usr/bin/env sh
# 安装脚本端到端自测（macOS / Linux）
#
# 用本地 HTTP 服务器模拟 Release，验证 install.sh 在 sh / bash / zsh 下的完整流程：
# 下载 → sha256 校验 → 安装 → 版本正确
#
# 用法：
#   scripts/e2e-install.sh [shell ...]     # 默认 sh
#   PORT=9000 scripts/e2e-install.sh zsh
#
# 依赖：python（提供静态 HTTP 服务）、已构建的 target/release/mtrim

set -eu

SHELLS="${*:-sh}"
BINARY="${BINARY:-target/release/mtrim}"
# 兼容 Windows 本地验证（CI 上不存在 .exe）
if [ ! -f "$BINARY" ] && [ -f "${BINARY}.exe" ]; then
    BINARY="${BINARY}.exe"
fi
PORT="${PORT:-8123}"

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"

case "$(uname -s)" in
    Darwin) OS="macos" ;;
    Linux)  OS="linux" ;;
    *) echo "❌ 不支持的平台：$(uname -s)" >&2; exit 1 ;;
esac

ARCH="$(uname -m)"
case "$ARCH" in
    arm64|aarch64) ARCH="aarch64" ;;
    x86_64|amd64)  ARCH="x86_64" ;;
    *) echo "❌ 不支持的架构：$ARCH" >&2; exit 1 ;;
esac

ASSET="mtrim-${OS}-${ARCH}"
PYTHON="$(command -v python3 || command -v python)"

if [ ! -f "$BINARY" ]; then
    echo "❌ 找不到 $BINARY，请先运行 cargo build --release" >&2
    exit 1
fi

ROOT="$(mktemp -d)"
DOWNLOAD="${ROOT}/releases/latest/download"
mkdir -p "$DOWNLOAD"
# 用重定向写入，避免 Windows/MSYS 给无扩展名文件自动追加 .exe
cat "$BINARY" > "${DOWNLOAD}/${ASSET}"

if command -v sha256sum >/dev/null 2>&1; then
    ( cd "$DOWNLOAD" && sha256sum "$ASSET" > sha256sums.txt )
else
    ( cd "$DOWNLOAD" && shasum -a 256 "$ASSET" > sha256sums.txt )
fi

# 用 cd 进入根目录再启动服务，避免 Windows 上 /tmp 路径解析问题
( cd "$ROOT" && exec "$PYTHON" -m http.server "$PORT" ) >/dev/null 2>&1 &
SERVER_PID=$!
cleanup() {
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
    rm -rf "$ROOT" 2>/dev/null || true
}
trap cleanup 0

BASE="http://127.0.0.1:${PORT}"
i=0
while [ "$i" -lt 40 ]; do
    if curl -fsS -o /dev/null "${BASE}/releases/latest/download/sha256sums.txt" 2>/dev/null; then
        break
    fi
    i=$((i + 1))
    sleep 0.5
done

for SHELL_BIN in $SHELLS; do
    TARGET_DIR="${ROOT}/bin-${SHELL_BIN}"
    echo "→ 使用 ${SHELL_BIN} 安装"
    MONO_TRIM_MIRROR="$BASE" "$SHELL_BIN" scripts/install.sh --dir "$TARGET_DIR" --force
    ACTUAL="$("${TARGET_DIR}/mtrim" --version)"
    if [ "$ACTUAL" != "mtrim ${VERSION}" ]; then
        echo "❌ 版本不符：期望 mtrim ${VERSION}，实际 ${ACTUAL}" >&2
        exit 1
    fi
    echo "✅ ${SHELL_BIN} 安装流程通过"

    echo "→ 使用 ${SHELL_BIN} 执行卸载"
    "$SHELL_BIN" scripts/uninstall.sh --dir "$TARGET_DIR" >/dev/null
    if [ -e "${TARGET_DIR}/mtrim" ]; then
        echo "❌ 卸载后仍存在 ${TARGET_DIR}/mtrim" >&2
        exit 1
    fi
    echo "✅ ${SHELL_BIN} 卸载流程通过"
done
