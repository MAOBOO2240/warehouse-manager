// Release 版不顯示主控台視窗
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod engine;

fn main() {
    // 啟動時於 .exe 同層建立 projects/ 與預設專案；失敗時交由前端顯示原因
    let root = engine::storage::projects_root_from_exe().and_then(|root| {
        engine::storage::ensure_root(&root)?;
        Ok(root)
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::AppState::new(root))
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_projects,
            commands::create_project,
            commands::delete_project,
            commands::open_project,
            commands::read_data,
            commands::write_data_atomic,
            commands::apply_transaction,
            commands::restore_backup,
            commands::export_text_file,
            commands::export_project_zip,
            commands::import_project_zip,
        ])
        .run(tauri::generate_context!())
        .expect("無法啟動倉儲管理系統");
}
