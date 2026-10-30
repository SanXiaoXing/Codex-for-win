use crate::{
    commands,
    model::{initial, UsageState},
    providers, tray,
    window::SavedPosition,
};
use std::sync::{atomic::AtomicBool, Mutex};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(UsageState(Mutex::new(initial())))
        .manage(SavedPosition(Mutex::new(None)))
        .manage(AtomicBool::new(false))
        .setup(|app| {
            crate::window::configure_main_window(app);
            tray::initialize(app)?;
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    providers::refresh(&app_handle).await;
                    tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_usage,
            commands::refresh_usage,
            commands::save_workbuddy_session,
            commands::save_trae_session,
            commands::save_trae_authorization,
            commands::open_trae_dashboard,
            commands::open_atomcode_page,
            crate::foreground::get_foreground_provider,
            crate::window::set_ball_mode
        ])
        .run(tauri::generate_context!())
        .expect("error while running Win CodexBar");
}
