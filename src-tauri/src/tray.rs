use crate::model::UsageSnapshot;
use crate::providers;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

pub(crate) fn tray_tooltip(snapshot: &UsageSnapshot) -> String {
    let codex = snapshot
        .codex
        .windows
        .iter()
        .map(|window| {
            format!(
                "{} 剩余 {}%",
                window.label,
                100_u8.saturating_sub(window.used_percent)
            )
        })
        .collect::<Vec<_>>();
    let workbuddy = snapshot
        .workbuddy
        .credits
        .as_ref()
        .map(|credits| format!("WorkBuddy {:.0} 积分", credits.remaining))
        .unwrap_or_else(|| "WorkBuddy 未连接".into());
    let trae = snapshot
        .trae
        .credits
        .as_ref()
        .map(|credits| format!("TRAE 剩余 {:.0} 积分", credits.remaining))
        .unwrap_or_else(|| "TRAE 未连接".into());
    if codex.is_empty() {
        format!("Win CodexBar · Codex 未连接 · {workbuddy} · {trae}")
    } else {
        format!("Win CodexBar · {} · {workbuddy} · {trae}", codex.join(" / "))
    }
}

pub(crate) fn initialize(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let open = MenuItem::with_id(app, "open", "打开用量面板", true, None::<&str>)?;
    let refresh_item = MenuItem::with_id(app, "refresh", "刷新用量", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &refresh_item, &quit])?;
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    TrayIconBuilder::with_id("usage-tray")
        .icon(icon)
        .tooltip("Win CodexBar · AI 用量")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_panel(app),
            "refresh" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    providers::refresh(&app).await;
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_panel(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn show_panel(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit("restore-panel", ());
    }
}
