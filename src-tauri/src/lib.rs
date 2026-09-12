use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use txtbatch::{process_dir, Operation, Summary, DEFAULT_CTX};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangeRequest {
    dir: String,
    mode: String,
    find: String,
    replace_with: String,
    after: String,
    insert_text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChangeResponse {
    files_scanned: usize,
    files_modified: usize,
    total_edits: usize,
    binary_skipped: usize,
    details: Vec<FileResponse>,
    errors: Vec<ErrorResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileResponse {
    path: String,
    diffs: Vec<DiffResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffResponse {
    line_no: usize,
    old: String,
    new: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    path: String,
    message: String,
}

fn operation_from(request: &ChangeRequest) -> Result<Operation, String> {
    match request.mode.as_str() {
        "replace" if !request.find.is_empty() => Ok(Operation::Replace {
            find: request.find.clone(),
            replace: request.replace_with.clone(),
        }),
        "insert" if !request.after.is_empty() => Ok(Operation::Insert {
            after: request.after.clone(),
            insert: request.insert_text.clone(),
        }),
        "replace" => Err("查找文本不能为空".to_string()),
        "insert" => Err("定位文本不能为空".to_string()),
        _ => Err("未知操作模式".to_string()),
    }
}

fn execute(request: ChangeRequest, dry_run: bool) -> Result<ChangeResponse, String> {
    let dir = PathBuf::from(&request.dir);
    if !dir.is_dir() {
        return Err(format!("目录不存在: {}", request.dir));
    }
    let operation = operation_from(&request)?;
    let summary =
        process_dir(&dir, &operation, dry_run, DEFAULT_CTX).map_err(|error| error.to_string())?;
    Ok(response_from(summary))
}

fn response_from(summary: Summary) -> ChangeResponse {
    ChangeResponse {
        files_scanned: summary.files_scanned,
        files_modified: summary.files_modified,
        total_edits: summary.total_edits,
        binary_skipped: summary.binary_skipped,
        details: summary
            .details
            .into_iter()
            .map(|file| FileResponse {
                path: file.path.display().to_string(),
                diffs: file
                    .diffs
                    .into_iter()
                    .map(|diff| DiffResponse {
                        line_no: diff.line_no,
                        old: diff.start,
                        new: diff.new,
                    })
                    .collect(),
            })
            .collect(),
        errors: summary
            .errors
            .into_iter()
            .map(|(path, message)| ErrorResponse {
                path: path.display().to_string(),
                message,
            })
            .collect(),
    }
}

#[tauri::command]
fn load_saved_dir() -> Result<Option<String>, String> {
    txtbatch::config::load_dir().map_err(|error| error.to_string())
}

#[tauri::command]
fn save_directory(dir: String) -> Result<(), String> {
    txtbatch::config::save_dir(&dir).map_err(|error| error.to_string())
}

#[tauri::command]
async fn preview_changes(request: ChangeRequest) -> Result<ChangeResponse, String> {
    tauri::async_runtime::spawn_blocking(move || execute(request, true))
        .await
        .map_err(|error| format!("预览任务失败: {error}"))?
}

#[tauri::command]
async fn apply_changes(request: ChangeRequest) -> Result<ChangeResponse, String> {
    tauri::async_runtime::spawn_blocking(move || execute(request, false))
        .await
        .map_err(|error| format!("应用任务失败: {error}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            load_saved_dir,
            save_directory,
            preview_changes,
            apply_changes
        ])
        .run(tauri::generate_context!())
        .expect("error while running txtbatch Tauri application");
}
