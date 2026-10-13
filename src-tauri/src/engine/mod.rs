//! 與 Tauri 無關的核心邏輯（可獨立單元測試）。

pub mod error;
pub mod storage;
pub mod tx;

use error::{AppError, AppResult};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::fs;
use std::io::Write;
use std::path::Path;
use storage::{read_json, write_json_atomic, APP_STATE_FILE, DATA_FILES};

#[derive(Serialize, Debug)]
pub struct FileIssue {
    pub file: String,
    pub code: &'static str,
    pub message: String,
    pub has_backup: bool,
}

#[derive(Serialize, Debug)]
pub struct OpenedProject {
    pub name: String,
    /// 各資料檔內容；讀取失敗的檔案為 null
    pub data: Map<String, Value>,
    pub issues: Vec<FileIssue>,
    /// 是否補完了上次中斷的交易
    pub recovered: bool,
}

/// 開啟專案：補完中斷的交易、讀取全部資料檔、回報損壞或被鎖定的檔案。
pub fn open_project(dir: &Path, name: &str) -> AppResult<OpenedProject> {
    let recovered = tx::commit_pending(dir).unwrap_or(false);
    let mut data = Map::new();
    let mut issues = Vec::new();
    for stem in DATA_FILES {
        match read_json(dir, stem) {
            Ok(v) => {
                let shape_ok = if stem == "config" { v.is_object() } else { v.is_array() };
                if shape_ok {
                    data.insert(stem.into(), v);
                } else {
                    data.insert(stem.into(), Value::Null);
                    issues.push(FileIssue {
                        file: stem.into(),
                        code: "FILE_CORRUPT",
                        message: format!("{stem}.json 內容格式不正確"),
                        has_backup: storage::latest_valid_backup(dir, stem).is_some(),
                    });
                }
            }
            Err(e) => {
                data.insert(stem.into(), Value::Null);
                issues.push(FileIssue {
                    file: stem.into(),
                    code: e.code,
                    message: e.message,
                    has_backup: storage::latest_valid_backup(dir, stem).is_some(),
                });
            }
        }
    }
    Ok(OpenedProject {
        name: name.into(),
        data,
        issues,
        recovered,
    })
}

pub fn read_last_project(root: &Path) -> Option<String> {
    read_json(root, APP_STATE_FILE)
        .ok()
        .and_then(|v| v.get("last_project").and_then(Value::as_str).map(String::from))
}

pub fn save_last_project(root: &Path, name: &str) -> AppResult<()> {
    write_json_atomic(root, APP_STATE_FILE, &json!({ "last_project": name }), false)
}

/// 將專案資料夾（含 backups/）打包為 zip，壓縮檔內最上層為專案名稱資料夾。
pub fn zip_project(dir: &Path, name: &str, dest: &Path) -> AppResult<()> {
    use zip::write::SimpleFileOptions;
    let io = |e: std::io::Error| AppError::io("匯出 ZIP 失敗", &e);
    let zerr = |e: zip::result::ZipError| AppError::new("IO_ERROR", format!("匯出 ZIP 失敗：{e}"));

    let tmp = dest.with_extension("zip.tmp");
    let file = fs::File::create(&tmp).map_err(io)?;
    let mut zw = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut stack = vec![dir.to_path_buf()];
    while let Some(cur) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&cur).map_err(io)?.flatten().map(|e| e.path()).collect();
        entries.sort();
        for path in entries {
            let rel = path.strip_prefix(dir).unwrap_or(&path);
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let zip_name = format!("{name}/{rel_str}");
            if path.is_dir() {
                zw.add_directory(format!("{zip_name}/"), opts.clone()).map_err(zerr)?;
                stack.push(path);
            } else {
                if rel_str.ends_with(".tmp") {
                    continue;
                }
                let bytes = fs::read(&path).map_err(io)?;
                zw.start_file(zip_name, opts.clone()).map_err(zerr)?;
                zw.write_all(&bytes).map_err(io)?;
            }
        }
    }
    let file = zw.finish().map_err(zerr)?;
    file.sync_all().map_err(io)?;
    drop(file);
    fs::rename(&tmp, dest).map_err(io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::storage::tests_support::temp_root;
    use super::*;

    #[test]
    fn open_reports_corrupt_files_without_crashing() {
        let root = temp_root("open");
        storage::create_project(&root, "p").unwrap();
        let dir = storage::project_dir(&root, "p").unwrap();
        write_json_atomic(&dir, "products", &json!([{"id":"PROD-001","name":"a"}]), true).unwrap();
        write_json_atomic(&dir, "products", &json!([{"id":"PROD-001","name":"b"}]), true).unwrap();
        fs::write(storage::file_path(&dir, "products"), b"").unwrap();
        let opened = open_project(&dir, "p").unwrap();
        assert_eq!(opened.issues.len(), 1);
        assert_eq!(opened.issues[0].file, "products");
        assert!(opened.issues[0].has_backup);
        assert!(opened.data["products"].is_null());
        assert!(opened.data["inventory"].is_array());
    }

    #[test]
    fn zip_contains_project_files() {
        let root = temp_root("zip");
        storage::create_project(&root, "分店倉").unwrap();
        let dir = storage::project_dir(&root, "分店倉").unwrap();
        let dest = root.join("out.zip");
        zip_project(&dir, "分店倉", &dest).unwrap();
        let mut z = zip::ZipArchive::new(fs::File::open(&dest).unwrap()).unwrap();
        let names: Vec<String> = (0..z.len())
            .map(|i| z.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains(&"分店倉/products.json".to_string()), "{names:?}");
        assert!(names.contains(&"分店倉/backups/".to_string()), "{names:?}");
    }

    #[test]
    fn last_project_roundtrip() {
        let root = temp_root("state");
        assert_eq!(read_last_project(&root), None);
        save_last_project(&root, "分店倉").unwrap();
        assert_eq!(read_last_project(&root).as_deref(), Some("分店倉"));
    }
}
