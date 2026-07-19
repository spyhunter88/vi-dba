mod ai;
mod commands;
mod config_manager;
mod db;
mod db_manager;
mod history_manager;
mod models;
mod script_manager;
mod settings_manager;

use db_manager::DbManager;
use settings_manager::SettingsManager;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle();
            let settings_manager = SettingsManager::new(app_handle);

            app.manage(settings_manager);

            // If the user configured a custom Oracle client directory, make its OCI
            // libraries discoverable by prepending it to the process PATH before any
            // Oracle connection (and thus any OCI load) can happen.
            if let Some(dir) = app.state::<SettingsManager>().load_settings().oracle_lib_dir {
                let dir = dir.trim();
                if !dir.is_empty() {
                    let sep = if cfg!(windows) { ";" } else { ":" };
                    let current = std::env::var("PATH").unwrap_or_default();
                    std::env::set_var("PATH", format!("{}{}{}", dir, sep, current));
                }
            }

            app.manage(DbManager::new());
            app.manage(config_manager::ConfigManager::new());
            app.manage(script_manager::ScriptManager::new());
            app.manage(ai::ai_manager::AiManager::new());

            let mut schema_cache_manager =
                db::schema_cache_manager::SchemaCacheManager::new(app_handle);

            // Initialize schema cache sync
            let _ = tauri::async_runtime::block_on(async { schema_cache_manager.init().await });

            app.manage(schema_cache_manager);

            // Install sqlx any drivers
            sqlx::any::install_default_drivers();

            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(if cfg!(debug_assertions) {
                        log::LevelFilter::Info
                    } else {
                        log::LevelFilter::Error
                    })
                    .build(),
            )?;
            app.handle().plugin(tauri_plugin_dialog::init())?;
            app.handle().plugin(tauri_plugin_fs::init())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::disconnect,
            commands::execute_query,
            commands::execute_script,
            commands::cancel_query,
            commands::get_objects,
            commands::test_connection,
            commands::get_connections,
            commands::save_connection,
            commands::delete_connection,
            commands::get_table_list,
            commands::get_routine_list,
            commands::update_row,
            commands::insert_row,
            commands::get_table_definition,
            commands::get_table_indexes,
            commands::create_table,
            commands::alter_table,
            commands::generate_table_sql,
            commands::list_scripts,
            commands::save_script,
            commands::read_script,
            commands::get_routine_definition,
            commands::save_routine,
            commands::get_view_definition,
            commands::save_view,
            commands::open_settings,
            commands::open_history,
            commands::open_import,
            commands::open_export,
            commands::set_export_data,
            commands::get_stashed_export_info,
            commands::get_file_preview,
            commands::perform_import,
            commands::perform_export,
            commands::open_export_multi,
            commands::perform_multi_import,
            commands::get_app_settings,
            commands::update_app_settings,
            commands::refresh_schema_cache,
            commands::get_schema_cache,
            commands::clear_schema_cache,
            commands::clear_all_schema_cache,
            history_manager::save_session,
            history_manager::load_session,
            history_manager::add_query_history,
            history_manager::get_query_history,
            history_manager::get_query_history_for_tab,
            history_manager::clear_query_history,
            history_manager::save_snapshot,
            history_manager::get_snapshots,
            history_manager::read_snapshot,
            history_manager::get_all_snapshots_summary,
            commands::generate_ai_sql,
            commands::list_local_models,
            commands::cancel_ai_generation,
            commands::open_models_directory,
            commands::test_ollama_connection
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
