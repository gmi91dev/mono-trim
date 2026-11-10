#!/usr/bin/env sh
# 卸载 mtrim（macOS / Linux）
#
# 用法：
#   uninstall.sh [--dir ~/.local/bin] [--keep-path] [--dry-run]
#
# 说明：
#   - 只删除安装目录下的 mtrim 可执行文件，目录为空时一并删除；
#   - 安装脚本不会修改 shell 配置，因此这里只提示 PATH 可能残留的写法；
#   - 项目配置（.mono-trim/config.toml、mono-trim.toml）不会被删除。

set -eu

DIR="${HOME}/.local/bin"
KEEP_PATH=0
DRY_RUN=0

usage() {
    sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
}

while [ $# -gt 0 ]; do
    case "$1" in
        --dir) DIR="${2:?--dir 需要参数}"; shift 2 ;;
        --keep-path) KEEP_PATH=1; shift ;;
        --dry-run) DRY_RUN=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "❌ 未知参数：$1" >&2; usage; exit 1 ;;
    esac
done

TARGET="${DIR}/mtrim"

echo "→ 安装目录: ${DIR}"

if [ ! -e "$TARGET" ]; then
    echo "ℹ️ 未在 ${DIR} 找到 mtrim"
else
    if [ "$DRY_RUN" -eq 1 ]; then
        echo "（DryRun）将删除 ${TARGET}"
    else
        rm -f "$TARGET"
        echo "✅ 已删除 ${TARGET}"
    fi
fi

# 目录为空时一并删除（仅当确实是我们的安装目录）
if [ "$DRY_RUN" -eq 0 ] && [ -d "$DIR" ]; then
    if [ -z "$(ls -A "$DIR" 2>/dev/null)" ]; then
        rmdir "$DIR" 2>/dev/null && echo "✅ 已删除空目录 ${DIR}"
    fi
fi

if [ "$KEEP_PATH" -eq 0 ]; then
    case ":${PATH}:" in
        *":${DIR}:"*)
            echo "ℹ️ ${DIR} 仍在当前 PATH 中；如已写入 ~/.bashrc / ~/.zshrc，请手动删除对应 export 行"
            ;;
    esac
fi

echo "ℹ️ 项目配置（.mono-trim/config.toml、mono-trim.toml）不会被删除"
