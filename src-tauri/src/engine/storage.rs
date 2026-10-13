//! 檔案儲存層：專案目錄管理、原子寫入、輪轉備份、備份還原。
//!
//! 原子寫入流程（write_json_atomic）：
//!   1. 舊檔非空時，先複製到 backups/<名稱>_<時間戳>.bak，並只保留最新 5 份
//!   2. 新內容寫入 <名稱>.json.tmp，呼叫 sync_all() 強制刷入硬碟
//!   3. std::fs::rename 以 .tmp 原子替換正式檔（遇到防毒或索引程式短暫鎖檔會重試）
//! 任何時間點斷電，正式檔只會是「完整舊版」或「完整新版」，不會出現 0 位元組檔。

use super::error::{is_lock_error, AppError, AppResult};
use chrono::{Local, Utc};
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

pub const PROJECTS_DIR: &str = "projects";
pub const BACKUPS_DIR: &str = "backups";
pub const DEFAULT_PROJECT: &str = "預設總倉";
pub const BACKUP_KEEP: usize = 5;
pub const APP_STATE_FILE: &str = "_app_state";
pub const PENDING_FILE: &str = "pending_tx";

/// 專案內的資料檔（不含副檔名）
pub const DATA_FILES: [&str; 5] = ["config", "products", "locations", "inventory", "logs"];
/// 前端可以用通用寫入 API 直接覆寫的檔案；inventory / logs 只能經由交易 API 異動
pub const MASTER_FILES: [&str; 3] = ["config", "products", "locations"];

pub fn validate_file_type(file_type: &str) -> AppResult<()> {
    if DATA_FILES.contains(&file_type) {
        Ok(())
    } else {
        Err(AppError::invalid(format!("未知的資料檔類型：{file_type}")))
    }
}

/// 專案名稱即資料夾名稱，需符合 Windows 檔名規則。
pub fn validate_project_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::invalid("專案名稱不可空白"));
    }
    if name.chars().count() > 50 {
        return Err(AppError::invalid("專案名稱請在 50 個字以內"));
    }
    const BAD: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    if name.chars().any(|c| BAD.contains(&c) || c.is_control()) {
        return Err(AppError::invalid("專案名稱不可包含 < > : \" / \\ | ? * 等符號"));
    }
    if name.starts_with('.') || name.starts_with('_') || name.ends_with('.') || name.ends_with(' ') {
        return Err(AppError::invalid("專案名稱不可以「.」或「_」開頭，也不可以「.」結尾"));
    }
    let upper = name.to_ascii_uppercase();
    let base = upper.split('.').next().unwrap_or("");
    let reserved = ["CON", "PRN", "AUX", "NUL"];
    let is_reserved = reserved.contains(&base)
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base.as_bytes()[3].is_ascii_digit());
    if is_reserved {
        return Err(AppError::invalid("此名稱為 Windows 保留字，請換一個名稱"));
    }
    Ok(name.to_string())
}

/// 程式所在資料夾下的 projects/
pub fn projects_root_from_exe() -> AppResult<PathBuf> {
    let exe = std::env::current_exe().map_err(|e| AppError::io("無法取得程式所在位置", &e))?;
    let dir = exe
        .parent()
        .ok_or_else(|| AppError::not_found("無法取得程式所在資料夾"))?;
    Ok(dir.join(PROJECTS_DIR))
}

/// 啟動時呼叫：建立 projects/，若沒有任何專案則建立預設專案。
pub fn ensure_root(root: &Path) -> AppResult<()> {
    fs::create_dir_all(root).map_err(|e| {
        AppError::new(
            "ROOT_NOT_WRITABLE",
            format!(
                "無法在程式所在資料夾建立 projects 資料夾（{e}）。\n請將 WarehouseManager.exe 移到桌面、文件或隨身碟等可寫入的位置後再開啟。"
            ),
        )
    })?;
    // 實際寫入測試，避免資料夾存在但唯讀（例如放在 Program Files）
    let probe = root.join(".write_test");
    fs::write(&probe, b"ok").map_err(|e| {
        AppError::new(
            "ROOT_NOT_WRITABLE",
            format!("projects 資料夾無法寫入（{e}）。請將程式移到可寫入的位置後再開啟。"),
        )
    })?;
    let _ = fs::remove_file(&probe);
    if list_projects(root)?.is_empty() {
        init_project(root, DEFAULT_PROJECT)?;
    }
    Ok(())
}

pub fn list_projects(root: &Path) -> AppResult<Vec<String>> {
    let mut names = Vec::new();
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(names),
        Err(e) => return Err(AppError::io("讀取專案清單失敗", &e)),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if !name.starts_with('.') && !name.starts_with('_') {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

pub fn project_dir(root: &Path, project: &str) -> AppResult<PathBuf> {
    let name = validate_project_name(project)?;
    let dir = root.join(name);
    if !dir.is_dir() {
        return Err(AppError::not_found(format!("找不到專案「{project}」")));
    }
    Ok(dir)
}

/// 建立新專案資料夾與 5 個資料檔、backups/ 目錄。已存在則回傳錯誤。
pub fn create_project(root: &Path, project: &str) -> AppResult<String> {
    let name = validate_project_name(project)?;
    let dir = root.join(&name);
    if dir.exists() {
        return Err(AppError::invalid(format!("專案「{name}」已存在")));
    }
    init_project(root, &name)?;
    Ok(name)
}

/// 初始化（或補齊）專案目錄結構；已存在的檔案不會被覆蓋。
pub fn init_project(root: &Path, name: &str) -> AppResult<()> {
    let dir = root.join(name);
    fs::create_dir_all(dir.join(BACKUPS_DIR)).map_err(|e| AppError::io("建立專案資料夾失敗", &e))?;
    for stem in DATA_FILES {
        let path = file_path(&dir, stem);
        if path.exists() {
            continue;
        }
        let initial = if stem == "config" {
            json!({
                "name": name,
                "created_at": now_utc(),
                "schema_version": 1
            })
        } else {
            json!([])
        };
        write_json_atomic(&dir, stem, &initial, false)?;
    }
    Ok(())
}

pub fn file_path(dir: &Path, stem: &str) -> PathBuf {
    dir.join(format!("{stem}.json"))
}

pub fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// 讀取 JSON。檔案不存在時：config 回傳 {}，其餘回傳 []。
/// 檔案為 0 位元組或格式錯誤 → FILE_CORRUPT；被其他程式鎖定 → FILE_LOCKED。
pub fn read_json(dir: &Path, stem: &str) -> AppResult<Value> {
    let path = file_path(dir, stem);
    let bytes = match fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(if stem == "config" { json!({}) } else { json!([]) });
        }
        Err(e) => return Err(AppError::io(&format!("讀取 {stem}.json 失敗"), &e)),
    };
    parse_json_bytes(&bytes).map_err(|detail| AppError::corrupt(format!("{stem}.json 內容損壞（{detail}）")))
}

fn parse_json_bytes(bytes: &[u8]) -> Result<Value, String> {
    // 容忍被記事本加上的 UTF-8 BOM
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    if bytes.iter().all(|b| b.is_ascii_whitespace()) {
        return Err("檔案是空的".into());
    }
    serde_json::from_slice::<Value>(bytes).map_err(|e| format!("JSON 格式錯誤：{e}"))
}

/// 讀取並確認是陣列
pub fn read_array(dir: &Path, stem: &str) -> AppResult<Vec<Value>> {
    match read_json(dir, stem)? {
        Value::Array(items) => Ok(items),
        _ => Err(AppError::corrupt(format!("{stem}.json 內容格式不正確（應為陣列）"))),
    }
}

/// 原子寫入。`backup = true` 時會先備份舊檔並輪轉。
pub fn write_json_atomic(dir: &Path, stem: &str, value: &Value, backup: bool) -> AppResult<()> {
    let target = file_path(dir, stem);
    let tmp = dir.join(format!("{stem}.json.tmp"));
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| AppError::invalid(format!("資料序列化失敗：{e}")))?;

    if backup {
        backup_file(dir, stem)?;
    }

    {
        let mut f = fs::File::create(&tmp).map_err(|e| AppError::io(&format!("寫入 {stem}.json 暫存檔失敗"), &e))?;
        f.write_all(&bytes)
            .map_err(|e| AppError::io(&format!("寫入 {stem}.json 暫存檔失敗"), &e))?;
        f.sync_all()
            .map_err(|e| AppError::io(&format!("寫入 {stem}.json 暫存檔失敗"), &e))?;
    }

    rename_with_retry(&tmp, &target).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        AppError::io(&format!("替換 {stem}.json 失敗"), &e)
    })?;
    sync_dir(dir);
    Ok(())
}

/// Windows 上防毒軟體、搜尋索引常會短暫開啟檔案，導致 rename 失敗；短暫重試即可。
fn rename_with_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut last_err = None;
    for attempt in 0..10u64 {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if is_lock_error(&e) => {
                last_err = Some(e);
                thread::sleep(Duration::from_millis(50 * (attempt + 1)));
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::other("rename failed")))
}

#[cfg(unix)]
fn sync_dir(dir: &Path) {
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) {}

fn backup_stamp() -> String {
    Local::now().format("%Y%m%d_%H%M%S_%3f").to_string()
}

/// 將現有檔案複製到 backups/，並只保留最新 BACKUP_KEEP 份。0 位元組檔不備份。
fn backup_file(dir: &Path, stem: &str) -> AppResult<()> {
    let src = file_path(dir, stem);
    match fs::metadata(&src) {
        Ok(m) if m.len() > 0 => {}
        _ => return Ok(()),
    }
    let bdir = dir.join(BACKUPS_DIR);
    fs::create_dir_all(&bdir).map_err(|e| AppError::io("建立備份資料夾失敗", &e))?;
    let mut dest = bdir.join(format!("{stem}_{}.bak", backup_stamp()));
    let mut n = 1;
    while dest.exists() {
        dest = bdir.join(format!("{stem}_{}_{n}.bak", backup_stamp()));
        n += 1;
    }
    fs::copy(&src, &dest).map_err(|e| AppError::io(&format!("備份 {stem}.json 失敗"), &e))?;
    rotate_backups(dir, stem);
    Ok(())
}

/// 列出某資料檔的所有備份，新到舊排序
pub fn list_backups(dir: &Path, stem: &str) -> Vec<PathBuf> {
    let bdir = dir.join(BACKUPS_DIR);
    let prefix = format!("{stem}_");
    let mut files: Vec<PathBuf> = fs::read_dir(&bdir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| {
                            n.starts_with(&prefix)
                                && n.ends_with(".bak")
                                && n[prefix.len()..n.len() - 4]
                                    .chars()
                                    .all(|c| c.is_ascii_digit() || c == '_')
                        })
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files.reverse();
    files
}

fn rotate_backups(dir: &Path, stem: &str) {
    for old in list_backups(dir, stem).into_iter().skip(BACKUP_KEEP) {
        let _ = fs::remove_file(old);
    }
}

/// 最近一份「內容可正常解析」的備份
pub fn latest_valid_backup(dir: &Path, stem: &str) -> Option<PathBuf> {
    list_backups(dir, stem)
        .into_iter()
        .find(|p| fs::read(p).ok().map(|b| parse_json_bytes(&b).is_ok()).unwrap_or(false))
}

/// 以最近一份有效備份覆蓋正本。損壞的正本另存為 backups/<名稱>_corrupt_<時間>.json 供日後查驗。
/// 回傳使用的備份檔名。
pub fn restore_from_backup(dir: &Path, stem: &str) -> AppResult<String> {
    let backup = latest_valid_backup(dir, stem)
        .ok_or_else(|| AppError::new("NO_BACKUP", format!("找不到 {stem}.json 可用的備份檔")))?;
    let bytes = fs::read(&backup).map_err(|e| AppError::io("讀取備份失敗", &e))?;
    let value = parse_json_bytes(&bytes).map_err(|e| AppError::corrupt(e))?;

    let target = file_path(dir, stem);
    if target.exists() {
        let keep = dir
            .join(BACKUPS_DIR)
            .join(format!("{stem}_corrupt_{}.json", backup_stamp()));
        let _ = fs::copy(&target, keep);
    }
    write_json_atomic(dir, stem, &value, false)?;
    Ok(backup
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string())
}

#[cfg(test)]
pub mod tests_support {
    use super::*;
    pub fn temp_root(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "wm_test_{tag}_{}_{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::temp_root;
    use super::*;

    #[test]
    fn creates_default_project_with_all_files() {
        let root = temp_root("init");
        ensure_root(&root).unwrap();
        assert_eq!(list_projects(&root).unwrap(), vec![DEFAULT_PROJECT.to_string()]);
        let dir = project_dir(&root, DEFAULT_PROJECT).unwrap();
        for f in DATA_FILES {
            assert!(file_path(&dir, f).exists(), "{f} missing");
        }
        assert!(dir.join(BACKUPS_DIR).is_dir());
        assert_eq!(read_json(&dir, "config").unwrap()["name"], DEFAULT_PROJECT);
    }

    #[test]
    fn rejects_bad_project_names() {
        for bad in ["", "  ", "a/b", "..", "CON", "com1", "x?y", "_hidden", "a."] {
            assert!(validate_project_name(bad).is_err(), "{bad:?} should be rejected");
        }
        assert_eq!(validate_project_name(" 分店倉 ").unwrap(), "分店倉");
    }

    #[test]
    fn atomic_write_keeps_five_rotating_backups() {
        let root = temp_root("rot");
        create_project(&root, "測試").unwrap();
        let dir = project_dir(&root, "測試").unwrap();
        for i in 0..9 {
            write_json_atomic(&dir, "products", &json!([{ "i": i }]), true).unwrap();
        }
        assert_eq!(read_json(&dir, "products").unwrap(), json!([{ "i": 8 }]));
        assert_eq!(list_backups(&dir, "products").len(), BACKUP_KEEP);
        assert!(!dir.join("products.json.tmp").exists());
        // 最新的備份應是上一版
        let latest = fs::read(&list_backups(&dir, "products")[0]).unwrap();
        assert_eq!(parse_json_bytes(&latest).unwrap(), json!([{ "i": 7 }]));
    }

    #[test]
    fn detects_corruption_and_restores_latest_valid_backup() {
        let root = temp_root("restore");
        create_project(&root, "p").unwrap();
        let dir = project_dir(&root, "p").unwrap();
        write_json_atomic(&dir, "inventory", &json!([{ "v": 1 }]), true).unwrap();
        write_json_atomic(&dir, "inventory", &json!([{ "v": 2 }]), true).unwrap();
        // 模擬斷電後的 0 位元組檔
        fs::write(file_path(&dir, "inventory"), b"").unwrap();
        assert_eq!(read_json(&dir, "inventory").unwrap_err().code, "FILE_CORRUPT");
        // 模擬一份壞掉的最新備份，應被略過
        fs::write(dir.join(BACKUPS_DIR).join("inventory_99999999_999999_999.bak"), b"{bad").unwrap();
        let used = restore_from_backup(&dir, "inventory").unwrap();
        assert_ne!(used, "inventory_99999999_999999_999.bak");
        assert_eq!(read_json(&dir, "inventory").unwrap(), json!([{ "v": 1 }]));
    }

    #[test]
    fn restore_without_backup_reports_no_backup() {
        let root = temp_root("nobak");
        create_project(&root, "p").unwrap();
        let dir = project_dir(&root, "p").unwrap();
        fs::write(file_path(&dir, "logs"), b"[{").unwrap();
        assert_eq!(restore_from_backup(&dir, "logs").unwrap_err().code, "NO_BACKUP");
    }
}
