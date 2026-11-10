//! 系统 CLI 自更新：从 GitHub Release 下载与当前平台匹配的产物。
//!
//! 下载与替换都通过 trait 抽象，便于在没有网络（或没有真实 Release）时测试决策逻辑。

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Result;
use crate::util::make_executable;

/// 发布仓库
pub const REPO: &str = "gmi91dev/mono-trim";

/// 已下载的最新版产物
pub struct Fetched {
    pub version: String,
    pub binary: PathBuf,
}

/// 获取最新版（网络实现或测试替身）
pub trait Fetcher {
    fn fetch_latest(&self) -> Result<Fetched>;
}

/// 用新产物替换当前可执行文件
pub trait Replacer {
    fn replace(&self, new_binary: &Path) -> Result<()>;
}

/// 当前平台对应的产物名，例如 `mtrim-win-x86_64.exe`
pub fn asset_name() -> String {
    let os = match std::env::consts::OS {
        "windows" => "win",
        "macos" => "macos",
        other => other,
    };
    let ext = if cfg!(windows) { ".exe" } else { "" };
    format!("mtrim-{os}-{}{ext}", std::env::consts::ARCH)
}

pub fn latest_url(asset: &str) -> String {
    format!("https://github.com/{REPO}/releases/latest/download/{asset}")
}

/// 通过 GitHub Release 的 `latest/download` 直链下载（无需解析 API）
pub struct GithubFetcher;

impl Fetcher for GithubFetcher {
    fn fetch_latest(&self) -> Result<Fetched> {
        let asset = asset_name();
        let dest = std::env::temp_dir().join(format!("{asset}.new"));
        let _ = std::fs::remove_file(&dest);

        download(&latest_url(&asset), &dest)?;
        make_executable(&dest);

        let version = read_version(&dest)?;
        Ok(Fetched {
            version,
            binary: dest,
        })
    }
}

fn download(url: &str, dest: &Path) -> Result<()> {
    // 失败时统一给出简洁提示，避免把各平台本地化的命令输出直接抛给用户
    let hint = || format!("下载 {url} 失败，请检查网络或手动从 Release 安装");
    // 优先 curl（Win10+ / macOS / Linux 通常自带）
    if let Ok(output) = Command::new("curl")
        .args(["-fsSL", "--retry", "2", "-o"])
        .arg(dest)
        .arg(url)
        .output()
        && output.status.success()
    {
        return Ok(());
    }

    #[cfg(windows)]
    {
        let script = format!(
            "Invoke-WebRequest -UseBasicParsing -Uri '{url}' -OutFile '{}'",
            dest.display()
        );
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .map_err(|e| format!("无法执行下载命令：{e}"))?;
        if output.status.success() {
            return Ok(());
        }
        Err(hint())
    }

    #[cfg(not(windows))]
    Err(hint())
}

fn read_version(binary: &Path) -> Result<String> {
    let output = Command::new(binary)
        .arg("--version")
        .output()
        .map_err(|e| format!("无法执行下载的产物：{e}"))?;
    if !output.status.success() {
        return Err("下载的产物无法运行（--version 失败）".into());
    }

    // 期望输出：`mtrim 0.2.0`
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.split_whitespace()
        .nth(1)
        .map(str::to_string)
        .filter(|version| crate::version::is_valid(version))
        .ok_or_else(|| format!("无法识别的版本输出：{}", text.trim()))
}

/// 替换当前正在运行的可执行文件
pub struct SelfReplacer;

impl Replacer for SelfReplacer {
    fn replace(&self, new_binary: &Path) -> Result<()> {
        let current = std::env::current_exe().map_err(|e| format!("无法定位当前程序：{e}"))?;
        make_executable(new_binary);

        // Windows 上运行中的 exe 不能被覆盖：先改名，再写入，失败则回滚
        let file_name = current
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "mtrim".to_string());
        let backup = current.with_file_name(format!("{file_name}.old-{}", std::process::id()));

        if cfg!(windows) {
            std::fs::rename(&current, &backup).map_err(|e| format!("无法替换当前程序（{e}）"))?;
            if let Err(e) = std::fs::copy(new_binary, &current) {
                let _ = std::fs::rename(&backup, &current);
                return Err(format!("写入新版本失败（{e}），已回滚"));
            }
            let _ = std::fs::remove_file(&backup);
            return Ok(());
        }

        let staged = current.with_file_name(format!("{file_name}.new-{}", std::process::id()));
        std::fs::copy(new_binary, &staged).map_err(|e| format!("无法写入新版本（{e}）"))?;
        make_executable(&staged);
        std::fs::rename(&staged, &current).map_err(|e| format!("无法替换当前程序（{e}）"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_name_uses_current_platform() {
        let asset = asset_name();
        assert!(asset.starts_with("mtrim-"), "{asset}");
        assert!(asset.contains(std::env::consts::ARCH), "{asset}");
        assert_eq!(asset.ends_with(".exe"), cfg!(windows), "{asset}");
    }

    #[test]
    fn latest_url_points_to_release_download() {
        assert_eq!(
            latest_url("mtrim-win-x86_64.exe"),
            "https://github.com/gmi91dev/mono-trim/releases/latest/download/mtrim-win-x86_64.exe"
        );
    }
}
