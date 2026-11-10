//! `init`：生成配置文件，并按平台部署二进制产物。

use clap::ArgMatches;
use std::fs;
use std::path::Path;

use crate::Result;
use crate::cli::{COMMAND, LEGACY_COMMAND, PLATFORMS};
use crate::config::{CONFIG_NAME, TOOL_DIR, display_path, template};
use crate::git::{SystemGit, project_root};
use crate::util::{dedupe, make_executable, write_file};

pub fn run(sub_matches: &ArgMatches) -> Result<()> {
    println!("⏳ 正在初始化 mono-trim 封闭环境...");
    let tool_dir = project_root(&SystemGit).join(TOOL_DIR);
    fs::create_dir_all(&tool_dir).map_err(|e| format!("创建 {TOOL_DIR} 目录失败：{e}"))?;

    let config_path = tool_dir.join(CONFIG_NAME);
    if config_path.exists() {
        println!("ℹ️ 已存在 {}，保留原有配置。", display_path());
    } else {
        write_file(&config_path, &template())?;
    }

    deploy_binaries(&tool_dir, &selected_platforms(sub_matches));

    println!("✨ 初始化成功！配置文件已生成，具体配置方法请直接查看该文件：");
    println!("👉 {}", display_path());
    println!("⚠️  修改配置后请务必运行一次 `{COMMAND} doctor` 校验，再执行自定义裁剪命令。");
    Ok(())
}

/// 计算需要部署的平台：`--plat` 作为初始集，`--exc` 从中剔除
fn selected_platforms(sub_matches: &ArgMatches) -> Vec<String> {
    let mut platforms: Vec<String> = match sub_matches.get_many::<String>("platforms") {
        Some(values) => values.cloned().collect(),
        None => PLATFORMS.iter().map(|s| s.to_string()).collect(),
    };
    if let Some(excludes) = sub_matches.get_many::<String>("exclude") {
        let excludes: Vec<String> = excludes.cloned().collect();
        platforms.retain(|p| !excludes.contains(p));
    }
    dedupe(&mut platforms);
    platforms
}

fn deploy_binaries(tool_dir: &Path, platforms: &[String]) {
    if platforms.is_empty() {
        println!("ℹ️ 所有平台均被排除，跳过多平台二进制部署。");
        return;
    }

    let current_exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            println!("⚠️ 无法定位当前程序路径（{e}），跳过二进制部署。");
            return;
        }
    };
    let Some(exe_dir) = current_exe.parent() else {
        println!("⚠️ 无法定位当前程序所在目录，跳过二进制部署。");
        return;
    };

    let host = host_platform();
    let mut deployed = 0usize;
    for plat in platforms {
        let Some((source_name, target_name)) = artifact_names(plat) else {
            continue;
        };

        let candidate = exe_dir.join(&source_name);
        let legacy = exe_dir.join(legacy_artifact_name(plat));
        let (source, note) = if candidate.exists() {
            (candidate, "")
        } else if legacy.exists() {
            // 兼容改名前的历史产物（mono-trim-win.exe 等）
            (legacy, "（兼容旧产物名）")
        } else if plat.as_str() == host {
            // 容错：产物未按平台命名，但当前平台刚好匹配，则复用自身
            (current_exe.clone(), "（复用当前可执行文件）")
        } else {
            println!("⚠️ 未找到 [{plat}] 端产物 {source_name}，已跳过。");
            continue;
        };

        let target = tool_dir.join(&target_name);
        match fs::copy(&source, &target) {
            Ok(_) => {
                make_executable(&target);
                deployed += 1;
                println!("⚙️  已成功部署 [{plat}] 端二进制文件 -> {TOOL_DIR}/{target_name}{note}");
            }
            Err(e) => println!("⚠️ 部署 [{plat}] 端产物失败：{e}"),
        }
    }

    if deployed == 0 {
        println!("⚠️ 未部署任何平台二进制文件，请将交叉编译产物与程序放在同一目录后重试。");
    }
}

/// 全局目录下按平台命名的源产物名（win 带 .exe 后缀）
fn source_artifact_name(command: &str, platform: &str) -> String {
    if platform == "win" {
        format!("{command}-win.exe")
    } else {
        format!("{command}-{platform}")
    }
}

/// 部署到 `.mono-trim/` 的目标产物名
fn target_artifact_name(platform: &str) -> String {
    if platform == "win" {
        format!("{COMMAND}.exe")
    } else {
        format!("{COMMAND}-{platform}")
    }
}

/// 部署时使用的产物名：(源文件名, 目标文件名)
fn artifact_names(platform: &str) -> Option<(String, String)> {
    matches!(platform, "win" | "mac" | "linux").then(|| {
        (
            source_artifact_name(COMMAND, platform),
            target_artifact_name(platform),
        )
    })
}

/// 改名前的历史产物名，仅用于兼容
fn legacy_artifact_name(platform: &str) -> String {
    source_artifact_name(LEGACY_COMMAND, platform)
}

fn host_platform() -> &'static str {
    match std::env::consts::OS {
        "windows" => "win",
        "macos" => "mac",
        "linux" => "linux",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli;

    fn init_matches(extra: &[&str]) -> ArgMatches {
        let mut argv = vec!["mtrim", "init"];
        argv.extend_from_slice(extra);
        cli::build().try_get_matches_from(argv).unwrap()
    }

    fn platforms(extra: &[&str]) -> Vec<String> {
        let matches = init_matches(extra);
        let (_, init_args) = matches.subcommand().unwrap();
        selected_platforms(init_args)
    }

    #[test]
    fn defaults_to_all_platforms() {
        assert_eq!(platforms(&[]), cli::PLATFORMS);
    }

    #[test]
    fn plat_selects_and_exc_removes() {
        assert_eq!(platforms(&["-p", "win", "linux"]), ["win", "linux"]);
        assert_eq!(platforms(&["-e", "mac"]), ["win", "linux"]);
        assert!(platforms(&["-p", "win", "-e", "win"]).is_empty());
    }

    #[test]
    fn dedupes_repeated_platforms() {
        assert_eq!(platforms(&["-p", "win", "win"]), ["win"]);
    }

    #[test]
    fn artifact_names_follow_command_name() {
        assert_eq!(
            artifact_names("win").unwrap(),
            ("mtrim-win.exe".to_string(), "mtrim.exe".to_string())
        );
        assert_eq!(
            artifact_names("mac").unwrap(),
            ("mtrim-mac".to_string(), "mtrim-mac".to_string())
        );
        assert!(artifact_names("android").is_none());
    }

    #[test]
    fn legacy_artifact_names_are_still_supported() {
        assert_eq!(legacy_artifact_name("win"), "mono-trim-win.exe");
        assert_eq!(legacy_artifact_name("linux"), "mono-trim-linux");
    }

    #[test]
    fn host_platform_is_known() {
        let host = host_platform();
        assert!(
            host.is_empty() || cli::PLATFORMS.contains(&host),
            "未知宿主平台：{host}"
        );
    }
}
