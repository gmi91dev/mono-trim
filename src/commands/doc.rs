//! `doc`：把 profiles 渲染成 Markdown 速查表并注入目标文档。

use std::fs;
use std::io;
use std::path::Path;

use crate::Result;
use crate::cli::COMMAND;
use crate::config::{self, Config, DEFAULT_DOC_TARGET, TOOL_DIR};
use crate::util::write_file;

/// 注入区块的起止标记，用于重复执行时原地更新而非无限追加
const DOC_BEGIN: &str = "<!-- mono-trim:begin -->";
const DOC_END: &str = "<!-- mono-trim:end -->";

pub fn run(config_path: &Path) -> Result<()> {
    let config = config::read_config(config_path)?;
    let relative_target = config.doc_target.as_deref().unwrap_or(DEFAULT_DOC_TARGET);
    // doc_target 相对配置文件所在目录（即 .mono-trim/）解析，"../README.md" 才指向项目根
    let tool_dir = config_path.parent().unwrap_or(Path::new("."));
    let target_path = tool_dir.join(relative_target);
    let block = render_block(&config);

    let existing = match fs::read_to_string(&target_path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("读取文档 '{relative_target}' 失败：{e}")),
    };

    if let Some(parent) = target_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建目录 {} 失败：{e}", parent.display()))?;
    }
    write_file(&target_path, &replace_block(&existing, &block))?;

    println!(
        "✨ 文档已更新至 {relative_target}（导出 {} 条自定义命令）。",
        config.profiles.len()
    );
    Ok(())
}

fn render_block(config: &Config) -> String {
    let mut block = String::from(DOC_BEGIN);
    block.push_str("\n## 🛠️ 项目工作空间裁剪命令\n\n");
    block.push_str("| 命令 | 模式 | 功能说明 |\n| :--- | :--- | :--- |\n");
    for name in config::sorted_profiles(config) {
        let profile = &config.profiles[name];
        block.push_str(&format!(
            "| `./{TOOL_DIR}/{COMMAND} {name}` | `{}` | {} |\n",
            profile.mode,
            escape_cell(&profile.desc)
        ));
    }
    block.push_str(&format!(
        "| `./{TOOL_DIR}/{COMMAND} reset` | `全量` | 恢复完整工作空间 |\n"
    ));
    block.push_str(DOC_END);
    block
}

/// 命中已有标记区块则原地替换，否则追加，保证 `doc` 可重复执行
fn replace_block(existing: &str, block: &str) -> String {
    let (head, tail) = match existing.find(DOC_BEGIN) {
        Some(start) => match existing[start..].find(DOC_END) {
            Some(offset) => {
                let end = start + offset + DOC_END.len();
                (existing[..start].trim(), existing[end..].trim())
            }
            // 区块标记残缺：直接舍弃到文件末尾，避免脏数据累积
            None => (existing[..start].trim(), ""),
        },
        None => (existing.trim(), ""),
    };

    let mut parts: Vec<&str> = Vec::with_capacity(3);
    if !head.is_empty() {
        parts.push(head);
    }
    parts.push(block);
    if !tail.is_empty() {
        parts.push(tail);
    }
    format!("{}\n", parts.join("\n\n"))
}

fn escape_cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_existing_block_in_place() {
        let block = format!("{DOC_BEGIN}\nNEW\n{DOC_END}");
        let existing = format!("# 标题\n\n{DOC_BEGIN}\nold\n{DOC_END}\n\n## 结尾\n");
        let updated = replace_block(&existing, &block);
        assert_eq!(updated, format!("# 标题\n\n{block}\n\n## 结尾\n"));
        // 幂等：再跑一次结果不变
        assert_eq!(replace_block(&updated, &block), updated);
    }

    #[test]
    fn drops_unterminated_block() {
        let block = format!("{DOC_BEGIN}\nNEW\n{DOC_END}");
        let existing = format!("# 标题\n\n{DOC_BEGIN}\n脏数据");
        assert_eq!(
            replace_block(&existing, &block),
            format!("# 标题\n\n{block}\n")
        );
    }

    #[test]
    fn appends_when_no_block() {
        assert_eq!(replace_block("# 标题\n", "NEW"), "# 标题\n\nNEW\n");
        assert_eq!(replace_block("", "NEW"), "NEW\n");
    }

    #[test]
    fn escapes_pipes_for_markdown_table() {
        assert_eq!(escape_cell("a | b\nc"), "a \\| b c");
    }
}
