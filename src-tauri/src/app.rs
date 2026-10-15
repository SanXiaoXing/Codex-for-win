use crate::{
    commands,
    model::{initial, UsageState},
    providers, tray,
    window::SavedPosition,
};
use std::sync::{atomic::AtomicBool, Mutex};

pub fn run() {
    tauri::Builder::default()
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
            commands::save_trae_authorization,
            commands::trae_checkin::save_trae_checkin_credentials,
            commands::trae_checkin::trae_checkin,
            commands::open_trae_dashboard,
            crate::window::set_ball_mode
        ])
        .run(tauri::generate_context!())
        .expect("error while running Win CodexBar");
}
