mod bridge;
mod core;
mod types;

use bridge::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            bridge::open_repo,
            bridge::close_repo,
            bridge::get_status,
            bridge::get_diff,
            bridge::stage_hunk,
            bridge::stage_lines,
            bridge::stage_file,
            bridge::unstage_file,
            bridge::discard,
            bridge::delete_file,
            bridge::commit,
            bridge::subscribe_watch,
            bridge::unsubscribe_watch,
            bridge::get_head_info,
            bridge::list_branches,
            bridge::switch_branch,
            bridge::get_log,
            bridge::get_commit_diff,
            bridge::search_files,
            bridge::list_backups,
            bridge::restore_from_backup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
