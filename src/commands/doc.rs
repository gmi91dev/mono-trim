//! `doc`：把用户命令渲染成 Markdown 速查表并注入目标文档。

use std::fs;
use std::io;

use crate::Result;
use crate::cli::COMMAND;
use crate::config::{Config, Project};
use crate::util::write_file;

/// 注入区块的起止标记，用于重复执行时原地更新而非无限追加
const DOC_BEGIN: &str = "<!-- mono-trim:begin -->";
const DOC_END: &str = "<!-- mono-trim:end -->";

pub fn run(project: &Project) -> Result<()> {
    if !project.is_initialized() {
        return Err(format!(
            "未发现系统配置 {}，请先运行 mtrim init",
            project.system_config_display()
        ));
    }

    let config = project.load()?;
    if let Some(hint) = config.version_hint() {
        println!("⚠️  {hint}");
    }

    let target_path = project.doc_target_path(&config);
    let block = render_block(&config);

    let existing = match fs::read_to_string(&target_path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("读取文档 '{}' 失败：{e}", config.doc_target)),
    };

    if let Some(parent) = target_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建目录 {} 失败：{e}", parent.display()))?;
    }
    write_file(&target_path, &replace_block(&existing, &block))?;

    println!(
        "✨ 文档已更新至 {}（导出 {} 条自定义命令）。",
        config.doc_target,
        config.profiles.len()
    );
    Ok(())
}

fn render_block(config: &Config) -> String {
    let mut block = String::from(DOC_BEGIN);
    block.push_str("\n## 🛠️ 项目工作空间裁剪命令\n\n");
    block.push_str("| 命令 | 模式 | 功能说明 |\n| :--- | :--- | :--- |\n");
    for name in config.sorted_profile_names() {
        let profile = &config.profiles[name];
        block.push_str(&format!(
            "| `{COMMAND} {name}` | `{}` | {} |\n",
            profile.mode,
            escape_cell(&profile.desc)
        ));
    }
    block.push_str(&format!(
        "| `{COMMAND} applyall` | `全量` | 恢复完整工作空间 |\n"
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
    use crate::test_support::TempDir;

    fn config_from(body: &str) -> Config {
        let user: crate::config::UserConfig = toml::from_str(body).unwrap();
        Config {
            version: "0.1.0".into(),
            doc_target: "README.md".into(),
            profiles: user.profiles,
        }
    }

    #[test]
    fn block_sorts_profiles_and_escapes_cells() {
        let config = config_from(
            r#"[profiles.zeta]
mode = "white"
desc = "含 | 管道符"

[profiles.alpha]
mode = "black"
desc = "普通说明"
"#,
        );

        let block = render_block(&config);
        assert!(block.starts_with(DOC_BEGIN));
        assert!(block.ends_with(DOC_END));
        assert!(block.contains("含 \\| 管道符"), "表格内管道符需转义");
        assert!(block.contains("mtrim applyall"));

        let alpha = block.find("mtrim alpha").unwrap();
        let zeta = block.find("mtrim zeta").unwrap();
        assert!(alpha < zeta, "命令应按名称排序输出");
    }

    #[test]
    fn run_writes_document_relative_to_project_root_and_is_idempotent() {
        let dir = TempDir::new("doc-run");
        dir.write(".mono-trim/config.toml", "version = \"0.1.0\"\n");
        dir.write(
            "mono-trim.toml",
            "doc_target = \"docs/commands.md\"\n\n[profiles.win-dev]\nmode = \"black\"\ndesc = \"隐藏 iOS 壳\"\n",
        );
        dir.write("docs/commands.md", "# 文档\n");
        let project = Project::from_root(dir.path());

        run(&project).unwrap();
        let first = dir.read("docs/commands.md");
        assert!(first.starts_with("# 文档"), "应保留原有内容");
        assert_eq!(first.matches(DOC_BEGIN).count(), 1);

        run(&project).unwrap();
        assert_eq!(
            dir.read("docs/commands.md"),
            first,
            "重复执行不应产生重复区块"
        );
    }

    #[test]
    fn run_creates_missing_document() {
        let dir = TempDir::new("doc-create");
        dir.write(".mono-trim/config.toml", "version = \"0.1.0\"\n");
        let project = Project::from_root(dir.path());

        run(&project).unwrap();

        let doc = dir.read("README.md");
        assert!(doc.contains(DOC_BEGIN));
        assert!(doc.contains("mtrim applyall"));
    }

    #[test]
    fn replace_block_behaviour() {
        let block = format!("{DOC_BEGIN}\nNEW\n{DOC_END}");
        let existing = format!("# 标题\n\n{DOC_BEGIN}\nold\n{DOC_END}\n\n## 结尾\n");
        let updated = replace_block(&existing, &block);
        assert_eq!(updated, format!("# 标题\n\n{block}\n\n## 结尾\n"));
        assert_eq!(replace_block(&updated, &block), updated, "应幂等");

        let broken = format!("# 标题\n\n{DOC_BEGIN}\n脏数据");
        assert_eq!(
            replace_block(&broken, &block),
            format!("# 标题\n\n{block}\n")
        );
    }

    #[test]
    fn escape_cell_replaces_pipes_and_newlines() {
        assert_eq!(escape_cell("a | b\nc"), "a \\| b c");
    }
}
