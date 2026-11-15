pub mod ai;
mod child_process;
pub mod commands;
pub mod error;
pub mod git;
pub mod operations;
pub mod settings;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;

            let app_data = app.path().app_data_dir()?;
            let settings_path = app_data.join("settings.json");
            let settings =
                tauri::async_runtime::block_on(crate::settings::SettingsStore::load(settings_path))
                    .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            let state = crate::commands::AppState::new(settings)
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_repository,
            commands::repository_status,
            commands::stage_selected,
            commands::stage_all,
            commands::unstage_selected,
            commands::read_diff,
            commands::commit,
            commands::pull,
            commands::push,
            commands::cancel_operation,
            commands::load_settings,
            commands::save_model_profile,
            commands::delete_model_profile,
            commands::set_active_model,
            commands::save_commit_rule,
            commands::test_model_profile,
            commands::generate_commit,
            commands::record_recent_repository,
            commands::diagnostics
        ])
        .run(tauri::generate_context!())
        .expect("failed to run SimpleGit");
}
