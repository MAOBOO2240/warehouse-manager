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
pub const DATA_FILES: [&str; 7] = [
    "config", "products", "locations", "inventory", "logs", "remarks", "row_order",
];
/// 前端可以用通用寫入 API 直接覆寫的檔案；inventory / logs 只能經由交易 API 異動
pub const MASTER_FILES: [&str; 5] = ["config", "products", "locations", "remarks", "row_order"];
/// 刪除的專案移到這裡（名稱以底線開頭，不會出現在專案清單），需要時可手動救回
pub const TRASH_DIR: &str = "_已刪除專案";

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

/// 刪除專案：整個資料夾移到 projects/_已刪除專案/<名稱>_<時間>，不直接抹除。
/// 若刪除後沒有任何專案，自動建立預設專案。回傳剩餘的專案清單。
pub fn delete_project(root: &Path, project: &str) -> AppResult<Vec<String>> {
    let dir = project_dir(root, project)?;
    let trash = root.join(TRASH_DIR);
    fs::create_dir_all(&trash).map_err(|e| AppError::io("建立已刪除專案資料夾失敗", &e))?;
    let stamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let mut dest = trash.join(format!("{project}_{stamp}"));
    let mut n = 1;
    while dest.exists() {
        dest = trash.join(format!("{project}_{stamp}_{n}"));
        n += 1;
    }
    rename_with_retry(&dir, &dest).map_err(|e| AppError::io(&format!("刪除專案「{project}」失敗"), &e))?;
    if list_projects(root)?.is_empty() {
        init_project(root, DEFAULT_PROJECT)?;
    }
    list_projects(root)
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

/// 從「匯出備份」產生的 ZIP 匯入成一個新專案，回傳新專案名稱。
///
/// - 壓縮檔可為「專案名稱/xxx.json」或直接把 json 放在最上層。
/// - 只取出專案資料檔、pending_tx.json 與 backups/ 內的檔案，其他內容一律略過（防止路徑穿越）。
/// - 名稱與既有專案重複時自動改名為「名稱 (2)」「名稱 (3)」…，原有專案完全不受影響。
/// - 先解壓到暫存資料夾，全部成功才改名成正式專案資料夾。
pub fn import_project_zip(root: &Path, zip_path: &Path) -> AppResult<String> {
    use std::io::Read;
    const MAX_ENTRY: u64 = 512 * 1024 * 1024;
    const MAX_TOTAL: u64 = 2 * 1024 * 1024 * 1024;
    let zerr = |e: zip::result::ZipError| AppError::invalid(format!("無法讀取 ZIP 檔：{e}"));
    let file = fs::File::open(zip_path).map_err(|e| AppError::io("開啟 ZIP 檔失敗", &e))?;
    let mut za = zip::ZipArchive::new(file).map_err(zerr)?;

    // 整理檔名（統一用 / 分隔），找出專案資料所在的前綴
    let names: Vec<String> = (0..za.len())
        .filter_map(|i| za.by_index(i).ok().map(|f| f.name().replace('\\', "/")))
        .collect();
    let is_data = |rel: &str| {
        rel.strip_suffix(".json")
            .is_some_and(|stem| DATA_FILES.contains(&stem) || stem == PENDING_FILE)
    };
    let prefix = if names.iter().any(|n| is_data(n)) {
        String::new()
    } else {
        let tops: std::collections::BTreeSet<&str> =
            names.iter().filter_map(|n| n.split_once('/').map(|(t, _)| t)).collect();
        tops.into_iter()
            .find(|t| names.iter().any(|n| n.strip_prefix(&format!("{t}/")).is_some_and(is_data)))
            .map(|t| format!("{t}/"))
            .ok_or_else(|| AppError::invalid("這個 ZIP 檔不是倉儲管理系統的專案備份（找不到 products.json 等資料檔）"))?
    };

    // 決定專案名稱：資料夾名稱 → ZIP 檔名
    let stem_name = zip_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("匯入專案")
        .to_string();
    let base = if prefix.is_empty() { stem_name } else { prefix.trim_end_matches('/').to_string() };
    let base = validate_project_name(&base).unwrap_or_else(|_| "匯入專案".to_string());
    let mut name = base.clone();
    let mut n = 2;
    while root.join(&name).exists() {
        let suffix = format!(" ({n})");
        let keep: String = base.chars().take(50 - suffix.chars().count()).collect();
        name = format!("{}{suffix}", keep.trim_end());
        n += 1;
    }

    let stamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let tmp = root.join(format!(".import_{stamp}_{}", std::process::id()));
    let result = (|| -> AppResult<()> {
        fs::create_dir_all(tmp.join(BACKUPS_DIR)).map_err(|e| AppError::io("建立暫存資料夾失敗", &e))?;
        let mut total = 0u64;
        for i in 0..za.len() {
            let mut entry = za.by_index(i).map_err(zerr)?;
            if entry.is_dir() {
                continue;
            }
            let full = entry.name().replace('\\', "/");
            let Some(rel) = full.strip_prefix(&prefix) else { continue };
            let safe = |f: &str| {
                !f.is_empty() && f != "." && f != ".." && !f.contains(['/', ':']) && !f.chars().any(char::is_control)
            };
            let dest = if is_data(rel) {
                tmp.join(rel)
            } else if let Some(f) = rel.strip_prefix("backups/") {
                if !safe(f) {
                    continue;
                }
                tmp.join(BACKUPS_DIR).join(f)
            } else {
                continue;
            };
            let mut buf = Vec::new();
            (&mut entry)
                .take(MAX_ENTRY + 1)
                .read_to_end(&mut buf)
                .map_err(|e| AppError::io("解壓縮失敗", &e))?;
            total += buf.len() as u64;
            if buf.len() as u64 > MAX_ENTRY || total > MAX_TOTAL {
                return Err(AppError::invalid("ZIP 檔內容過大，無法匯入"));
            }
            fs::write(&dest, &buf).map_err(|e| AppError::io("寫入匯入資料失敗", &e))?;
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&tmp);
        return Err(e);
    }
    let dir = root.join(&name);
    if let Err(e) = rename_with_retry(&tmp, &dir) {
        let _ = fs::remove_dir_all(&tmp);
        return Err(AppError::io("建立匯入專案失敗", &e));
    }
    // 補齊舊版備份沒有的資料檔，並把設定檔內的名稱改成新專案名稱
    init_project(root, &name)?;
    if let Ok(Value::Object(mut cfg)) = read_json(&dir, "config") {
        cfg.insert("name".into(), Value::String(name.clone()));
        write_json_atomic(&dir, "config", &Value::Object(cfg), false)?;
    }
    Ok(name)
}

/// 清除暫存：移除 projects 內殘留的 .tmp 暫存檔、未完成匯入的 .import_* 暫存資料夾與寫入測試檔。
/// 不會動到任何資料檔、backups/ 備份與 pending_tx.json（斷電補完用）。回傳清除的項目數。
pub fn clear_temp_files(root: &Path) -> AppResult<usize> {
    let mut removed = 0;
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(AppError::io("讀取專案資料夾失敗", &e)),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if name.starts_with(".import_") {
                if fs::remove_dir_all(&path).is_ok() {
                    removed += 1;
                }
                continue;
            }
            if name.starts_with('.') || name.starts_with('_') {
                continue;
            }
            for sub in [path.clone(), path.join(BACKUPS_DIR)] {
                let Ok(files) = fs::read_dir(&sub) else { continue };
                for f in files.flatten() {
                    let fp = f.path();
                    if fp.is_file() && fp.extension().is_some_and(|x| x == "tmp") && fs::remove_file(&fp).is_ok() {
                        removed += 1;
                    }
                }
            }
        } else if (name == ".write_test" || name.ends_with(".tmp")) && fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
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
    fn delete_project_moves_to_trash_and_keeps_one_project() {
        let root = temp_root("del");
        ensure_root(&root).unwrap();
        create_project(&root, "分店倉").unwrap();
        let left = delete_project(&root, "分店倉").unwrap();
        assert_eq!(left, vec![DEFAULT_PROJECT.to_string()]);
        let trashed: Vec<_> = fs::read_dir(root.join(TRASH_DIR)).unwrap().flatten().collect();
        assert_eq!(trashed.len(), 1);
        // 刪掉最後一個專案時自動建立預設專案
        let left = delete_project(&root, DEFAULT_PROJECT).unwrap();
        assert_eq!(left, vec![DEFAULT_PROJECT.to_string()]);
        assert!(delete_project(&root, "不存在").is_err());
    }

    #[test]
    fn import_zip_creates_new_project_without_touching_existing() {
        use std::io::Write as _;
        let root = temp_root("import");
        ensure_root(&root).unwrap();
        let zpath = root.join("..").join(format!("wm_import_{}.zip", std::process::id()));
        {
            let f = fs::File::create(&zpath).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let o = zip::write::SimpleFileOptions::default;
            zw.add_directory("預設總倉/", o()).unwrap();
            zw.start_file("預設總倉/products.json", o()).unwrap();
            zw.write_all(br#"[{"id":"PROD-001","name":"a"}]"#).unwrap();
            zw.start_file("預設總倉/config.json", o()).unwrap();
            zw.write_all(r#"{"name":"預設總倉","schema_version":1}"#.as_bytes()).unwrap();
            zw.start_file("預設總倉/backups/products.json.20260101_000000.bak", o()).unwrap();
            zw.write_all(b"[]").unwrap();
            zw.start_file("預設總倉/../evil.json", o()).unwrap();
            zw.write_all(b"x").unwrap();
            zw.start_file("預設總倉/notes.txt", o()).unwrap();
            zw.write_all(b"x").unwrap();
            zw.finish().unwrap();
        }
        let name = import_project_zip(&root, &zpath).unwrap();
        assert_eq!(name, "預設總倉 (2)");
        let dir = project_dir(&root, &name).unwrap();
        assert_eq!(read_json(&dir, "products").unwrap()[0]["name"], "a");
        assert_eq!(read_json(&dir, "config").unwrap()["name"], "預設總倉 (2)");
        assert!(file_path(&dir, "row_order").exists(), "缺少的資料檔應自動補齊");
        assert!(dir.join(BACKUPS_DIR).join("products.json.20260101_000000.bak").exists());
        assert!(!dir.join("notes.txt").exists());
        assert!(!root.join("evil.json").exists());
        // 原有專案不受影響
        assert_eq!(read_json(&project_dir(&root, DEFAULT_PROJECT).unwrap(), "products").unwrap(), json!([]));
        assert_eq!(list_projects(&root).unwrap().len(), 2);
        // 不是專案備份的 ZIP 會被拒絕
        let bad = root.join("..").join(format!("wm_bad_{}.zip", std::process::id()));
        {
            let mut zw = zip::ZipWriter::new(fs::File::create(&bad).unwrap());
            zw.start_file("hello.txt", zip::write::SimpleFileOptions::default()).unwrap();
            zw.write_all(b"hi").unwrap();
            zw.finish().unwrap();
        }
        assert!(import_project_zip(&root, &bad).is_err());
        assert_eq!(list_projects(&root).unwrap().len(), 2);
        let _ = fs::remove_file(zpath);
        let _ = fs::remove_file(bad);
    }

    #[test]
    fn clear_temp_files_keeps_real_data() {
        let root = temp_root("clear");
        ensure_root(&root).unwrap();
        let dir = project_dir(&root, DEFAULT_PROJECT).unwrap();
        write_json_atomic(&dir, "products", &json!([{"id":"PROD-001","name":"a"}]), true).unwrap();
        write_json_atomic(&dir, "products", &json!([{"id":"PROD-001","name":"b"}]), true).unwrap();
        fs::write(dir.join("products.json.tmp"), b"x").unwrap();
        fs::write(dir.join(PENDING_FILE.to_string() + ".json"), b"{}").unwrap();
        fs::create_dir_all(root.join(".import_20260101_000000_1")).unwrap();
        let backups_before = list_backups(&dir, "products").len();
        assert!(backups_before >= 1);
        assert_eq!(clear_temp_files(&root).unwrap(), 2);
        assert!(!dir.join("products.json.tmp").exists());
        assert!(dir.join(PENDING_FILE.to_string() + ".json").exists(), "斷電補完檔不可刪");
        assert_eq!(read_json(&dir, "products").unwrap()[0]["name"], "b");
        assert_eq!(list_backups(&dir, "products").len(), backups_before, "備份不可刪");
        assert!(!root.join(".import_20260101_000000_1").exists());
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
