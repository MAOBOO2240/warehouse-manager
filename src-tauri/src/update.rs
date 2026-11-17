//! 檢查更新：讀取 GitHub 上的 v1.0.0 發行版本（版本號固定 1.0.0，以編譯時的 commit 分新舊），
//! 有新版時下載 WarehouseManager.exe，和 WarehouseManager.sha256 比對無誤後替換執行檔並重新開啟。
//! 資料放在執行檔旁的 projects/ 資料夾，更新只換執行檔，不會動到任何資料。
//!
//! GitHub 倉庫是私人的時候，沒有登入查不到發行版本（GitHub 一律回 404）。這時在 GitHub Actions 設定
//! UPDATE_TOKEN 祕密（只給這個倉庫 Contents 唯讀的權杖），編譯時會包進執行檔，查詢與下載都帶著它；
//! 倉庫是公開的就不需要權杖。

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::AppHandle;

const REPO: &str = "MAOBOO2240/warehouse-manager";
/// GitHub 倉庫網址（設定裡的連結）
pub const REPO_URL: &str = "https://github.com/MAOBOO2240/warehouse-manager";
const TAG: &str = "v1.0.0";
const EXE_ASSET: &str = "WarehouseManager.exe";
const HASH_ASSET: &str = "WarehouseManager.sha256";
/// 權杖只會送到這裡（下載時 GitHub 轉到別的網域，ureq 轉址時不會帶著 Authorization）
const API_BASE: &str = "https://api.github.com/";

// 編譯時由 build.rs 產生：TOKEN_SEED、TOKEN_SUM、TOKEN_ENC（更新權杖打散後的內容；沒有設定時是空的）
include!(concat!(env!("OUT_DIR"), "/update_token.rs"));

/// 和 build.rs 的 fnv1a 相同：用來確認權杖還原得一模一樣
fn fnv1a(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in bytes {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// 還原編譯時包進來的更新權杖；沒有設定（倉庫公開）或還原結果不對時是空字串
fn token() -> String {
    let mut x = TOKEN_SEED;
    let bytes: Vec<u8> = TOKEN_ENC
        .iter()
        .map(|b| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            b ^ (x as u8)
        })
        .collect();
    if fnv1a(&bytes) != TOKEN_SUM {
        return String::new();
    }
    String::from_utf8(bytes).unwrap_or_default()
}

/// 這個執行檔是用哪個 commit 編譯的（GitHub Actions 編譯時才有；自己在電腦上編譯的版本是空的，不會自動更新）
pub fn current() -> &'static str {
    option_env!("WM_BUILD_SHA").unwrap_or("")
}

/// GitHub Actions 的編譯編號（顯示用）
fn build_no() -> &'static str {
    option_env!("WM_BUILD_NO").unwrap_or("")
}

/// 這次編譯的代號（GitHub Actions 的執行編號-第幾次嘗試）。同一個 commit 重新編譯時 commit 不變、代號會變——
/// 例如只換了更新權杖再編譯一次，已安裝的程式也要收得到。自己在電腦上編譯的版本是空的。
fn build_id() -> &'static str {
    option_env!("WM_BUILD_ID").unwrap_or("")
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
    /// 這個執行檔有沒有包進更新權杖（私人倉庫才需要）
    pub has_token: bool,
}

#[tauri::command]
pub fn update_info(app: AppHandle) -> UpdInfo {
    UpdInfo {
        version: app.package_info().version.to_string(),
        build: build_no().to_string(),
        commit: current().chars().take(7).collect(),
        can_update: !current().is_empty(),
        repo_url: REPO_URL.to_string(),
        has_token: !token().is_empty(),
    }
}

/// 查不到發行版本時的說明：私人倉庫沒有權杖、或權杖讀不到這個倉庫時，GitHub 都只回 404
fn not_found_message(has_token: bool) -> String {
    if has_token {
        "找不到發行版本（更新權杖可能沒有這個倉庫的讀取權限，或還沒有發布過版本）".to_string()
    } else {
        "找不到發行版本。GitHub 倉庫如果是私人的，需要先設定更新權杖（UPDATE_TOKEN）重新編譯，或把倉庫改成公開".to_string()
    }
}

/// 連線設定。逾時是「連上線」與「每一次讀取」各自的上限，不是整個下載的總時間：
/// 網路慢的時候，十幾 MB 的執行檔照樣下載得完；真的斷線才會中止。
fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(15))
        .redirects(5)
        // 下載附件時 GitHub 會轉到另一個網域的臨時網址：轉址後不可以再帶著權杖
        .redirect_auth_headers(ureq::RedirectAuthHeaders::Never)
        .user_agent("WarehouseManager-updater")
        .build()
}

const TOKEN_EXPIRED: &str = "更新權杖已失效（過期或被撤銷），請重新產生權杖、更新 GitHub 的 UPDATE_TOKEN 後重新編譯";

fn send(url: &str, accept: &str, token: Option<&str>) -> Result<ureq::Response, ureq::Error> {
    let mut req = agent()
        .get(url)
        .set("Accept", accept)
        .set("X-GitHub-Api-Version", "2022-11-28");
    if let Some(t) = token {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }
    req.call()
}

fn github(url: &str, accept: &str) -> Result<ureq::Response, String> {
    let token = token();
    // 權杖只送給 GitHub API，不送到其他網址
    let with_token = !token.is_empty() && url.starts_with(API_BASE);
    let mut result = send(url, accept, with_token.then_some(token.as_str()));
    // 權杖失效（過期或被撤銷）時，GitHub 連公開的資料也回 401：不帶權杖再試一次——
    // 倉庫已經改成公開的話照樣查得到、下載得到；還是不行才回報權杖失效
    let expired = with_token && matches!(result, Err(ureq::Error::Status(401, _)));
    if expired {
        result = send(url, accept, None);
    }
    // 下面的訊息依「這一次有沒有帶著權杖」來寫
    let auth = with_token && !expired;
    result.map_err(|e| match e {
        ureq::Error::Status(401, _) => TOKEN_EXPIRED.to_string(),
        // 權杖失效、不帶權杖又查不到：倉庫還是私人的，原因就是權杖失效
        ureq::Error::Status(404, _) if expired => TOKEN_EXPIRED.to_string(),
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
            } else if auth {
                "GitHub 拒絕了這次查詢（更新權杖的權限可能不夠），請稍後再試".to_string()
            } else {
                "GitHub 拒絕了這次查詢，請稍後再試".to_string()
            }
        }
        ureq::Error::Status(404, _) => not_found_message(auth),
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
    /// 新版本的編譯代號（發行版本說明裡的 build-id；舊的發行版本沒有，是空字串）
    pub latest_id: String,
    /// 和目前是同一個 commit，只是重新編譯過（例如換了更新權杖）
    pub rebuilt: bool,
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

/// 發行版本說明裡「build-id: 1234567890-1」的代號（GitHub Actions 的執行編號-第幾次嘗試）
fn build_id_of(notes: &str) -> String {
    notes
        .split("build-id:")
        .nth(1)
        .map(|s| {
            s.trim_start()
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '-')
                .collect()
        })
        .unwrap_or_default()
}

/// 有沒有新版：發行版本是用別的 commit 編譯的（commit 都是完整的 40 個字）
fn is_newer(current: &str, latest: &str) -> bool {
    current.len() == 40 && latest.len() == 40 && latest.chars().all(|c| c.is_ascii_hexdigit()) && !latest.eq_ignore_ascii_case(current)
}

/// 同一個 commit 重新編譯過：commit 一樣、編譯代號不一樣（兩邊都有代號才比；舊的發行版本沒有代號，不算）
fn is_rebuild(current: &str, latest: &str, current_id: &str, latest_id: &str) -> bool {
    current.len() == 40
        && latest.eq_ignore_ascii_case(current)
        && !current_id.is_empty()
        && !latest_id.is_empty()
        && current_id != latest_id
}

fn check_blocking() -> Result<Check, String> {
    let rel = release()?;
    let latest = rel["target_commitish"].as_str().unwrap_or("").to_string();
    let latest_id = build_id_of(rel["body"].as_str().unwrap_or(""));
    let cur = current();
    let rebuilt = is_rebuild(cur, &latest, build_id(), &latest_id);
    Ok(Check {
        has_update: is_newer(cur, &latest) || rebuilt,
        can_update: !cur.is_empty(),
        current: cur.chars().take(7).collect(),
        latest: latest.chars().take(7).collect(),
        latest_build: build_of(rel["name"].as_str().unwrap_or("")),
        latest_id,
        rebuilt,
        published: rel["published_at"].as_str().unwrap_or("").to_string(),
    })
}

#[tauri::command]
pub async fn update_check() -> Result<Check, String> {
    tauri::async_runtime::spawn_blocking(check_blocking).await.map_err(|e| e.to_string())?
}

/// 附件的下載網址。私人倉庫（有權杖）要從 API 的附件網址下載，一般的下載連結沒登入會是 404；
/// 公開倉庫用一般的下載連結，不佔用 API 的查詢次數。
fn asset_url<'a>(rel: &'a serde_json::Value, name: &str, private: bool) -> Option<&'a str> {
    let asset = rel["assets"].as_array()?.iter().find(|a| a["name"].as_str() == Some(name))?;
    let url = asset[if private { "url" } else { "browser_download_url" }].as_str()?;
    // 私人倉庫的附件網址一定要是 GitHub API（權杖只送到那裡）
    (!private || url.starts_with(API_BASE)).then_some(url)
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
    // 寫完強制刷入硬碟再換上去：換檔後馬上斷電，也不會留下內容不完整的程式檔
    let written = std::fs::File::create(&tmp).and_then(|mut f| {
        f.write_all(new_bytes)?;
        f.sync_all()
    });
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
        return Err("無法寫入新版本（程式所在的資料夾不能寫入）".into());
    }
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
    let private = !token().is_empty();
    let exe_url = asset_url(&rel, EXE_ASSET, private).ok_or("發行版本裡找不到 WarehouseManager.exe")?;
    let hash_url = asset_url(&rel, HASH_ASSET, private).ok_or("新版本缺少驗證資料，已取消更新")?;
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
pub async fn update_install(app: AppHandle, state: tauri::State<'_, crate::commands::AppState>) -> Result<(), String> {
    let exe = tauri::async_runtime::spawn_blocking(install_blocking).await.map_err(|e| e.to_string())??;
    // 等正在進行的資料寫入做完才關閉程式
    let writing = state.queue();
    std::process::Command::new(&exe)
        .spawn()
        .map_err(|_| "新版本已經下載好，但無法自動開啟，請自己重新開啟程式".to_string())?;
    // 寫入佇列不再放開：從這裡到程式真正結束之間，不會有新的寫入開始做到一半
    std::mem::forget(writing);
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
    fn rebuild_of_the_same_commit_counts_as_an_update() {
        // 發行版本說明裡的編譯代號
        assert_eq!(build_id_of("…資料不會受影響。\n\nbuild-id: 19283746501-2\n"), "19283746501-2");
        assert_eq!(build_id_of("build-id:77-1（重新編譯）"), "77-1");
        assert_eq!(build_id_of("舊的發行版本沒有代號"), "");
        let a = "0123456789abcdef0123456789abcdef01234567";
        let b = "89abcdef0123456789abcdef0123456789abcdef";
        // 同一個 commit、代號不同：重新編譯過（例如換了更新權杖），要更新
        assert!(is_rebuild(a, a, "100-1", "101-1"));
        assert!(is_rebuild(a, &a.to_uppercase(), "100-1", "100-2"));
        // 代號相同＝就是這一版；任何一邊沒有代號（舊的發行版本、舊的程式）不算
        assert!(!is_rebuild(a, a, "100-1", "100-1"));
        assert!(!is_rebuild(a, a, "", "101-1"));
        assert!(!is_rebuild(a, a, "100-1", ""));
        // commit 不同是「新版本」那一條在管；自己編譯的版本（沒有 commit）不會更新
        assert!(!is_rebuild(a, b, "100-1", "101-1"));
        assert!(!is_rebuild("", "", "100-1", "101-1"));
    }

    #[test]
    fn embedded_token_restores_exactly() {
        // build.rs 打散、這裡還原：兩邊的做法只要有一點不同，檢查碼就對不起來（token() 會變成空的）
        let restored = token();
        assert_eq!(restored.len(), TOKEN_ENC.len(), "還原後的權杖長度要和編譯時包進來的一樣");
        assert_eq!(fnv1a(restored.as_bytes()), TOKEN_SUM);
        // 編譯時有給 WM_UPDATE_TOKEN 的話，還原結果必須和它一模一樣
        if let Some(want) = option_env!("WM_UPDATE_TOKEN") {
            assert_eq!(restored, want.trim());
        }
        // 權杖不會以明文留在打散後的資料裡
        if restored.len() >= 8 {
            assert_ne!(&TOKEN_ENC[..], restored.as_bytes());
        }
    }

    #[test]
    fn asset_url_depends_on_repo_visibility() {
        let rel = serde_json::json!({ "assets": [
            { "name": "WarehouseManager.exe",
              "url": "https://api.github.com/repos/o/r/releases/assets/1",
              "browser_download_url": "https://github.com/o/r/releases/download/v1.0.0/WarehouseManager.exe" },
            { "name": "evil.exe", "url": "https://example.com/assets/2", "browser_download_url": "https://example.com/x" }
        ] });
        assert_eq!(
            asset_url(&rel, EXE_ASSET, false),
            Some("https://github.com/o/r/releases/download/v1.0.0/WarehouseManager.exe")
        );
        assert_eq!(asset_url(&rel, EXE_ASSET, true), Some("https://api.github.com/repos/o/r/releases/assets/1"));
        // 帶權杖下載時，網址不是 GitHub API 就不用
        assert_eq!(asset_url(&rel, "evil.exe", true), None);
        assert_eq!(asset_url(&rel, HASH_ASSET, false), None);
        assert!(not_found_message(false).contains("UPDATE_TOKEN"));
        assert!(!not_found_message(true).contains("改成公開"));
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
