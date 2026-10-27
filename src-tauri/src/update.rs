//! 檢查更新：讀取 GitHub 上的 v1.0.0 發行版本（版本號固定 1.0.0，以編譯時的 commit 分新舊），
//! 有新版時下載 WarehouseManager.exe，和 WarehouseManager.sha256 比對無誤後替換執行檔並重新開啟。
//! 資料放在執行檔旁的 projects/ 資料夾，更新只換執行檔，不會動到任何資料。

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::AppHandle;

const REPO: &str = "MAOBOO2240/warehouse-manager";
/// GitHub 倉庫網址（設定裡的連結）
pub const REPO_URL: &str = "https://github.com/MAOBOO2240/warehouse-manager";
const TAG: &str = "v1.0.0";
const EXE_ASSET: &str = "WarehouseManager.exe";
const HASH_ASSET: &str = "WarehouseManager.sha256";

/// 這個執行檔是用哪個 commit 編譯的（GitHub Actions 編譯時才有；自己在電腦上編譯的版本是空的，不會自動更新）
pub fn current() -> &'static str {
    option_env!("WM_BUILD_SHA").unwrap_or("")
}

/// GitHub Actions 的編譯編號（顯示用）
fn build_no() -> &'static str {
    option_env!("WM_BUILD_NO").unwrap_or("")
}

fn old_path(exe: &Path) -> PathBuf {
    let stem = exe.file_stem().and_then(|s| s.to_str()).unwrap_or("WarehouseManager");
    exe.with_file_name(format!("{stem}.old.exe"))
}

fn tmp_path(exe: &Path) -> PathBuf {
    let stem = exe.file_stem().and_then(|s| s.to_str()).unwrap_or("WarehouseManager");
    exe.with_file_name(format!("{stem}.update.exe"))
}

/// 更新後留下的舊執行檔、下載到一半的檔案，啟動時刪掉
pub fn cleanup() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(old_path(&exe));
        let _ = std::fs::remove_file(tmp_path(&exe));
    }
}

#[derive(Serialize)]
pub struct UpdInfo {
    pub version: String,
    /// 編譯編號（自己在電腦上編譯的是空字串）
    pub build: String,
    pub commit: String,
    /// 可以自動更新（GitHub 編譯的版本才可以）
    pub can_update: bool,
    pub repo_url: String,
}

#[tauri::command]
pub fn update_info(app: AppHandle) -> UpdInfo {
    UpdInfo {
        version: app.package_info().version.to_string(),
        build: build_no().to_string(),
        commit: current().chars().take(7).collect(),
        can_update: !current().is_empty(),
        repo_url: REPO_URL.to_string(),
    }
}

fn github(url: &str, accept: &str) -> Result<ureq::Response, String> {
    ureq::get(url)
        .set("Accept", accept)
        .set("User-Agent", "WarehouseManager-updater")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .timeout(Duration::from_secs(60))
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(403, r) | ureq::Error::Status(429, r) => {
                // 沒登入的查詢每小時每個網路位址 60 次
                let reset = r
                    .header("x-ratelimit-reset")
                    .and_then(|v| v.parse::<u64>().ok())
                    .and_then(|t| {
                        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
                        Some(t.saturating_sub(now) / 60 + 1)
                    });
                let remaining = r.header("x-ratelimit-remaining").map(str::to_string);
                if remaining.as_deref() == Some("0") {
                    format!("GitHub 暫時限制查詢次數，請 {} 分鐘後再試", reset.unwrap_or(60))
                } else {
                    "GitHub 拒絕了這次查詢，請稍後再試".to_string()
                }
            }
            ureq::Error::Status(404, _) => "找不到發行版本".to_string(),
            ureq::Error::Status(c, _) => format!("GitHub 回應錯誤（{c}）"),
            ureq::Error::Transport(_) => "連不上 GitHub，請確認網路後再試".to_string(),
        })
}

fn release() -> Result<serde_json::Value, String> {
    github(&format!("https://api.github.com/repos/{REPO}/releases/tags/{TAG}"), "application/vnd.github+json")?
        .into_json::<serde_json::Value>()
        .map_err(|_| "GitHub 的回應無法讀取".to_string())
}

#[derive(Serialize)]
pub struct Check {
    pub has_update: bool,
    /// 自己在電腦上編譯的版本不會自動更新
    pub can_update: bool,
    pub current: String,
    pub latest: String,
    /// 新版本的編譯編號（發行版本標題裡的 #數字）
    pub latest_build: String,
    pub published: String,
}

/// 發行版本標題「倉儲管理系統 v1.0.0（編譯 #12）」裡的編號
fn build_of(title: &str) -> String {
    title
        .split('#')
        .nth(1)
        .map(|s| s.chars().take_while(char::is_ascii_digit).collect())
        .unwrap_or_default()
}

/// 有沒有新版：發行版本是用別的 commit 編譯的（commit 都是完整的 40 個字）
fn is_newer(current: &str, latest: &str) -> bool {
    current.len() == 40 && latest.len() == 40 && latest.chars().all(|c| c.is_ascii_hexdigit()) && !latest.eq_ignore_ascii_case(current)
}

fn check_blocking() -> Result<Check, String> {
    let rel = release()?;
    let latest = rel["target_commitish"].as_str().unwrap_or("").to_string();
    let cur = current();
    Ok(Check {
        has_update: is_newer(cur, &latest),
        can_update: !cur.is_empty(),
        current: cur.chars().take(7).collect(),
        latest: latest.chars().take(7).collect(),
        latest_build: build_of(rel["name"].as_str().unwrap_or("")),
        published: rel["published_at"].as_str().unwrap_or("").to_string(),
    })
}

#[tauri::command]
pub async fn update_check() -> Result<Check, String> {
    tauri::async_runtime::spawn_blocking(check_blocking).await.map_err(|e| e.to_string())?
}

fn asset_url<'a>(rel: &'a serde_json::Value, name: &str) -> Option<&'a str> {
    rel["assets"].as_array()?.iter().find(|a| a["name"].as_str() == Some(name))?["browser_download_url"].as_str()
}

fn download(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    github(url, "application/octet-stream")?
        .into_reader()
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(|_| "下載中斷，請確認網路後再試".to_string())?;
    Ok(bytes)
}

/// 「雜湊值  檔名」格式的第一個欄位（64 個十六進位字元）
fn parse_hash(text: &str) -> Option<String> {
    let h = text.trim_start_matches('\u{feff}').split_whitespace().next()?.to_ascii_lowercase();
    (h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())).then_some(h)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// 執行中的檔案不能覆蓋，但可以改名：舊檔改名成 .old.exe → 新檔放到原位置；失敗就換回來
fn swap_exe(exe: &Path, new_bytes: &[u8]) -> Result<(), String> {
    let tmp = tmp_path(exe);
    std::fs::write(&tmp, new_bytes).map_err(|_| "無法寫入新版本（程式所在的資料夾不能寫入）".to_string())?;
    let old = old_path(exe);
    let _ = std::fs::remove_file(&old);
    if std::fs::rename(exe, &old).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return Err("無法替換程式檔，請關閉其他開著的倉儲管理系統後再試".into());
    }
    if std::fs::rename(&tmp, exe).is_err() {
        let _ = std::fs::rename(&old, exe);
        let _ = std::fs::remove_file(&tmp);
        return Err("無法替換程式檔，已還原成原本的版本".into());
    }
    Ok(())
}

/// 下載新版 → 和 GitHub 編譯時算好的雜湊值比對 → 替換執行檔
fn install_blocking() -> Result<PathBuf, String> {
    let rel = release()?;
    let exe_url = asset_url(&rel, EXE_ASSET).ok_or("發行版本裡找不到 WarehouseManager.exe")?;
    let hash_url = asset_url(&rel, HASH_ASSET).ok_or("新版本缺少驗證資料，已取消更新")?;
    let want = String::from_utf8(download(hash_url, 4096)?)
        .ok()
        .and_then(|t| parse_hash(&t))
        .ok_or("新版本的驗證資料格式不對，已取消更新")?;
    let bytes = download(exe_url, 200 * 1024 * 1024)?;
    // 下載不完整或檔案被改過：不更新
    if bytes.len() < 1024 || sha256_hex(&bytes) != want {
        return Err("下載的新版本驗證失敗（可能下載不完整），已取消更新，請稍後再試".into());
    }
    let exe = std::env::current_exe().map_err(|_| "找不到程式所在位置".to_string())?;
    swap_exe(&exe, &bytes)?;
    Ok(exe)
}

#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    let exe = tauri::async_runtime::spawn_blocking(install_blocking).await.map_err(|e| e.to_string())??;
    std::process::Command::new(&exe)
        .spawn()
        .map_err(|_| "新版本已經下載好，但無法自動開啟，請自己重新開啟程式".to_string())?;
    app.exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_file_format() {
        let h = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(parse_hash(&format!("{h}  WarehouseManager.exe")).as_deref(), Some(h));
        assert_eq!(parse_hash(&format!("\u{feff}{}\r\n", h.to_uppercase())).as_deref(), Some(h));
        assert_eq!(parse_hash("abc  WarehouseManager.exe"), None);
        assert_eq!(parse_hash(""), None);
        assert_eq!(sha256_hex(b""), h);
    }

    #[test]
    fn newer_only_for_real_commits() {
        let a = "0123456789abcdef0123456789abcdef01234567";
        let b = "89abcdef0123456789abcdef0123456789abcdef";
        assert!(is_newer(a, b));
        assert!(!is_newer(a, a));
        assert!(!is_newer(a, &a.to_uppercase()));
        // 自己編譯的版本（沒有 commit）、舊的發行版本（target 是分支名稱）都不算
        assert!(!is_newer("", b));
        assert!(!is_newer(a, "main"));
    }

    #[test]
    fn build_number_from_title() {
        assert_eq!(build_of("倉儲管理系統 v1.0.0（編譯 #12）"), "12");
        assert_eq!(build_of("v1.0.0 倉儲管理系統"), "");
    }

    #[test]
    fn swap_keeps_old_on_failure_and_replaces_on_success() {
        let dir = std::env::temp_dir().join(format!("wm_upd_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("WarehouseManager.exe");
        std::fs::write(&exe, b"old").unwrap();
        swap_exe(&exe, b"new").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
        assert_eq!(std::fs::read(old_path(&exe)).unwrap(), b"old");
        assert!(!tmp_path(&exe).exists());
        // 執行檔不見了（改名失敗）：不留下下載到一半的檔案
        std::fs::remove_file(&exe).unwrap();
        assert!(swap_exe(&exe, b"newer").is_err());
        assert!(!tmp_path(&exe).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
