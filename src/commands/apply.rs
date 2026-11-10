//! 裁剪相关的动态命令：自定义 profile、reset、reapply。

use std::path::Path;

use crate::Result;
use crate::config::{CONFIG_NAME, read_config};
use crate::git::{git_ok, sparse_checkout_path, sparse_enabled};
use crate::sparse::build_patterns;
use crate::util::write_file;

/// 执行 `config.toml` 中声明的自定义 profile（由命令名动态触发）
pub fn profile(config_path: &Path, name: &str) -> Result<()> {
    let config = read_config(config_path)?;
    let profile = config.profiles.get(name).ok_or_else(|| {
        format!(
            "未知命令 '{name}'。输入 --help 查看内置命令，或检查 {CONFIG_NAME} 中的 profiles 节点。"
        )
    })?;

    println!("⏳ 正在执行自定义命令 [{name}] ({}) ...", profile.desc);
    // 先完整校验并生成规则，任何非法配置都在触碰工作空间之前拦截
    let patterns = build_patterns(profile)?;

    git_ok(&["sparse-checkout", "init", "--no-cone"])?;
    write_file(&sparse_checkout_path()?, &patterns)?;
    git_ok(&["sparse-checkout", "reapply"])?;

    println!("✨ 工作空间拓扑裁剪成功！");
    Ok(())
}

pub fn reset() -> Result<()> {
    println!("⏳ 正在恢复全量目录与文件...");
    if !sparse_enabled()? {
        println!("✨ 当前工作空间本就是全量状态，无需恢复。");
        return Ok(());
    }

    // 优先使用官方 disable 彻底关闭稀疏检出，失败则退回全量规则
    if git_ok(&["sparse-checkout", "disable"]).is_err() {
        write_file(&sparse_checkout_path()?, "/*\n")?;
        git_ok(&["sparse-checkout", "reapply"])?;
    }

    println!("✨ 已成功恢复所有平台的完整工作空间！");
    Ok(())
}

pub fn reapply() -> Result<()> {
    if !sparse_enabled()? {
        return Err("当前工作空间尚未启用稀疏检出，请先执行自定义裁剪命令。".into());
    }
    println!("⏳ 正在重新刷新本地物理硬盘目录...");
    git_ok(&["sparse-checkout", "reapply"])?;
    println!("✨ 已按现有稀疏规则刷新完成！");
    Ok(())
}
