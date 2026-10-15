//! 前端可呼叫的 Tauri Command。所有檔案存取都在同一把鎖底下排隊執行（寫入佇列），
//! 避免快速連續點擊時讀寫互相衝突。

use crate::engine::{
    self,
    error::{AppError, AppResult},
    storage,
    tx::{self, TxOutcome, TxRequest},
    OpenedProject,
};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
#[allow(unused_imports)]
use tauri::Manager;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

pub struct AppState {
    root: AppResult<PathBuf>,
    lock: Mutex<()>,
}

impl AppState {
    pub fn new(root: AppResult<PathBuf>) -> Self {
        Self {
            root,
            lock: Mutex::new(()),
        }
    }
    fn root(&self) -> AppResult<PathBuf> {
        self.root.clone()
    }
    /// 寫入佇列：同一時間只會有一個檔案操作在進行
    fn queue(&self) -> MutexGuard<'_, ()> {
        self.lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    fn project_dir(&self, project: &str) -> AppResult<PathBuf> {
        storage::project_dir(&self.root()?, project)
    }
}

#[derive(Serialize)]
pub struct AppInfo {
    root: Option<String>,
    root_error: Option<AppError>,
    projects: Vec<String>,
    last_project: Option<String>,
    version: String,
}

#[tauri::command]
pub async fn get_app_info(app: AppHandle, state: State<'_, AppState>) -> AppResult<AppInfo> {
    let version = app.package_info().version.to_string();
    let _q = state.queue();
    Ok(match state.root() {
        Ok(root) => AppInfo {
            root: Some(root.to_string_lossy().to_string()),
            root_error: None,
            projects: storage::list_projects(&root)?,
            last_project: engine::read_last_project(&root),
            version,
        },
        Err(e) => AppInfo {
            root: None,
            root_error: Some(e),
            projects: vec![],
            last_project: None,
            version,
        },
    })
}

#[tauri::command]
pub async fn get_projects(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let _q = state.queue();
    storage::list_projects(&state.root()?)
}

#[tauri::command]
pub async fn create_project(name: String, state: State<'_, AppState>) -> AppResult<String> {
    let _q = state.queue();
    let root = state.root()?;
    let created = storage::create_project(&root, &name)?;
    let _ = engine::save_last_project(&root, &created);
    Ok(created)
}

/// 開啟專案並一次讀回全部資料（含損壞檔案清單）
#[tauri::command]
pub async fn open_project(project: String, state: State<'_, AppState>) -> AppResult<OpenedProject> {
    let _q = state.queue();
    let root = state.root()?;
    let dir = storage::project_dir(&root, &project)?;
    let opened = engine::open_project(&dir, &project)?;
    let _ = engine::save_last_project(&root, &project);
    Ok(opened)
}

#[tauri::command]
pub async fn read_data(project: String, file_type: String, state: State<'_, AppState>) -> AppResult<Value> {
    storage::validate_file_type(&file_type)?;
    let _q = state.queue();
    let dir = state.project_dir(&project)?;
    storage::read_json(&dir, &file_type)
}

/// 刪除專案（移到 projects/_已刪除專案），回傳剩餘專案清單
#[tauri::command]
pub async fn delete_project(project: String, state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let _q = state.queue();
    storage::delete_project(&state.root()?, &project)
}

/// 主檔寫入（商品 / 庫位 / 設定）。庫存與日誌只能經由 apply_transaction 異動。
#[tauri::command]
pub async fn write_data_atomic(
    project: String,
    file_type: String,
    content: Value,
    state: State<'_, AppState>,
) -> AppResult<()> {
    storage::validate_file_type(&file_type)?;
    if !storage::MASTER_FILES.contains(&file_type.as_str()) {
        return Err(AppError::invalid(format!(
            "{file_type}.json 不可直接寫入，請使用作業功能進行庫存異動"
        )));
    }
    let _q = state.queue();
    let dir = state.project_dir(&project)?;
    tx::validate_master_write(&dir, &file_type, &content)?;
    storage::write_json_atomic(&dir, &file_type, &content, true)
}

/// 庫存異動交易（入庫 / 出庫 / 調撥 / 盤點 / 報廢 / 損壞），後端二次校驗負庫存
#[tauri::command]
pub async fn apply_transaction(project: String, tx: TxRequest, state: State<'_, AppState>) -> AppResult<TxOutcome> {
    let _q = state.queue();
    let dir = state.project_dir(&project)?;
    tx::run_transaction(&dir, &tx)
}

/// 以最近一份有效備份還原指定資料檔，回傳使用的備份檔名
#[tauri::command]
pub async fn restore_backup(project: String, file_type: String, state: State<'_, AppState>) -> AppResult<String> {
    storage::validate_file_type(&file_type)?;
    let _q = state.queue();
    let dir = state.project_dir(&project)?;
    storage::restore_from_backup(&dir, &file_type)
}

fn dialog_path(picked: Option<tauri_plugin_dialog::FilePath>) -> AppResult<Option<PathBuf>> {
    match picked {
        None => Ok(None),
        Some(p) => p
            .into_path()
            .map(Some)
            .map_err(|e| AppError::new("IO_ERROR", format!("無法使用所選的路徑：{e}"))),
    }
}

/// 跳出「另存新檔」視窗並寫入文字檔（CSV 內容由前端產生，已含 UTF-8 BOM）。
/// 使用者按取消時回傳 None。
#[tauri::command]
pub async fn export_text_file(app: AppHandle, default_name: String, content: String) -> AppResult<Option<String>> {
    let picked = app
        .dialog()
        .file()
        .set_title("匯出 CSV")
        .set_file_name(default_name)
        .add_filter("CSV 檔案", &["csv"])
        .blocking_save_file();
    let Some(path) = dialog_path(picked)? else {
        return Ok(None);
    };
    fs::write(&path, content.as_bytes()).map_err(|e| AppError::io("寫入 CSV 失敗", &e))?;
    Ok(Some(path.to_string_lossy().to_string()))
}

/// 將目前專案資料夾打包為 .zip
#[tauri::command]
pub async fn export_project_zip(
    app: AppHandle,
    project: String,
    state: State<'_, AppState>,
) -> AppResult<Option<String>> {
    let dir = state.project_dir(&project)?;
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let picked = app
        .dialog()
        .file()
        .set_title("匯出專案備份")
        .set_file_name(format!("{project}_{stamp}.zip"))
        .add_filter("ZIP 壓縮檔", &["zip"])
        .blocking_save_file();
    let Some(path) = dialog_path(picked)? else {
        return Ok(None);
    };
    let _q = state.queue();
    engine::zip_project(&dir, &project, &path)?;
    Ok(Some(path.to_string_lossy().to_string()))
}

/// 選擇「匯出備份」產生的 .zip，匯入成一個新專案；回傳新專案名稱（取消時為 None）
#[tauri::command]
pub async fn import_project_zip(app: AppHandle, state: State<'_, AppState>) -> AppResult<Option<String>> {
    let picked = app
        .dialog()
        .file()
        .set_title("匯入專案備份")
        .add_filter("ZIP 壓縮檔", &["zip"])
        .blocking_pick_file();
    let Some(path) = dialog_path(picked)? else {
        return Ok(None);
    };
    let _q = state.queue();
    let name = storage::import_project_zip(&state.root()?, &path)?;
    Ok(Some(name))
}
