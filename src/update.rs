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

/// 下载源：默认 GitHub，可用 `MONO_TRIM_MIRROR` 指向内网镜像
pub fn release_base() -> String {
    std::env::var("MONO_TRIM_MIRROR")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("https://github.com/{REPO}"))
}

pub fn latest_url(asset: &str) -> String {
    format!("{}/releases/latest/download/{asset}", release_base())
}

/// 从 `sha256sums.txt` 中解析指定文件的校验和
/// 兼容 `hash  file`、`hash *file`、`hash ./file` 三种格式
pub fn parse_checksum(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let file = parts
            .next()?
            .trim_start_matches('*')
            .trim_start_matches("./");
        (file == name).then(|| hash.to_lowercase())
    })
}

/// 下载后的临时文件名（Windows 必须保留 .exe 才能执行）
fn download_file_name(asset: &str) -> String {
    if cfg!(windows) {
        format!("{asset}.download.exe")
    } else {
        format!("{asset}.download")
    }
}

/// 通过 GitHub Release 的 `latest/download` 直链下载（无需解析 API）
pub struct GithubFetcher;

impl Fetcher for GithubFetcher {
    fn fetch_latest(&self) -> Result<Fetched> {
        let asset = asset_name();
        let temp = std::env::temp_dir();
        let sums = temp.join(format!("mtrim-sums-{}.txt", std::process::id()));
        let dest = temp.join(download_file_name(&asset));
        let _ = std::fs::remove_file(&sums);
        let _ = std::fs::remove_file(&dest);

        // 先取校验和，再下载产物，避免下载后才发现校验失败
        download(&latest_url("sha256sums.txt"), &sums)?;
        let text =
            std::fs::read_to_string(&sums).map_err(|e| format!("读取校验和文件失败：{e}"))?;
        let expected = parse_checksum(&text, &asset)
            .ok_or_else(|| format!("校验文件中没有 {asset} 的记录"))?;
        let _ = std::fs::remove_file(&sums);

        download(&latest_url(&asset), &dest)?;

        let actual = crate::util::sha256_file(&dest)?;
        if actual != expected {
            let _ = std::fs::remove_file(&dest);
            return Err(format!("sha256 校验失败（期望 {expected}，实际 {actual}）"));
        }

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
        // 注意：存在 MONO_TRIM_MIRROR 时以镜像为准，这里只校验默认拼接规则
        let url = latest_url("mtrim-win-x86_64.exe");
        assert!(
            url.ends_with("/releases/latest/download/mtrim-win-x86_64.exe"),
            "{url}"
        );
        assert!(url.starts_with("http"), "{url}");
    }

    #[test]
    fn parses_checksums_in_all_formats() {
        let text = "aaa  *mtrim-win-x86_64.exe
bbb  ./mtrim-macos-aarch64
ccc  mtrim-linux-x86_64
";

        assert_eq!(
            parse_checksum(text, "mtrim-win-x86_64.exe").as_deref(),
            Some("aaa")
        );
        assert_eq!(
            parse_checksum(text, "mtrim-macos-aarch64").as_deref(),
            Some("bbb")
        );
        assert_eq!(
            parse_checksum(text, "mtrim-linux-x86_64").as_deref(),
            Some("ccc")
        );
        assert_eq!(parse_checksum(text, "missing"), None);
    }

    #[test]
    fn download_name_keeps_extension_on_windows() {
        let name = download_file_name("mtrim-win-x86_64.exe");
        if cfg!(windows) {
            assert!(name.ends_with(".exe"), "{name}");
        } else {
            assert!(name.ends_with(".download"), "{name}");
        }
    }
}
