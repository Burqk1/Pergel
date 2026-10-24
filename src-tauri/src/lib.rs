mod access;
mod code_scan;
mod live;
mod files;
mod fonts;
mod mcp;

use std::sync::{Arc, Mutex};

use serde_json::Value;
use tauri::{Manager, State};

struct McpState {
    bridge: Arc<mcp::Bridge>,
    status: Mutex<Option<mcp::Status>>,
}

/// The editor window answering an MCP tool call.
#[tauri::command]
fn mcp_respond(state: State<'_, McpState>, id: u64, reply: Value) {
    state.bridge.respond(id, reply);
}

/// Where the MCP server listens, for the UI.
#[tauri::command]
fn mcp_status(state: State<'_, McpState>) -> Option<mcp::Status> {
    state.status.lock().ok().and_then(|s| s.clone())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(files::Store::default())
        .manage(files::Picked::default())
        .setup(|app| {
            let bridge = Arc::new(mcp::Bridge::new(app.handle().clone()));
            app.manage(McpState { bridge: bridge.clone(), status: Mutex::new(None) });
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let status = mcp::serve(bridge).await;
                if let Some(state) = handle.try_state::<McpState>()
                    && let Ok(mut s) = state.status.lock()
                {
                    *s = Some(status);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            live::live_start,
            live::live_stop,
            mcp_respond,
            mcp_status,
            files::files_list,
            files::file_create,
            files::file_read,
            files::file_save,
            files::file_thumbnail,
            files::file_update,
            files::file_duplicate,
            files::file_delete,
            files::file_import,
            files::file_export,
            files::write_file,
            files::pick_path,
            files::file_versions,
            files::file_version_save,
            files::file_version_read,
            fonts::system_fonts,
            fonts::system_font_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pergel");
}
