use crate::model::{ProviderUsage, UsageSnapshot};
use crate::providers;
use crate::tray_icon::{render_quota_icon, ICON_SIZE};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

fn quota_levels(snapshot: &UsageSnapshot) -> [Option<u8>; 4] {
    let window_remaining = |usage: &ProviderUsage| {
        if usage.status != "connected" {
            return None;
        }
        usage
            .windows
            .iter()
            .find(|window| window.label == "5 小时")
            .or_else(|| usage.windows.first())
            .map(|window| 100_u8.saturating_sub(window.used_percent))
    };
    let credit_remaining = |usage: &ProviderUsage| {
        if usage.status != "connected" {
            return None;
        }
        let credits = usage.credits.as_ref()?;
        if !credits.remaining.is_finite() || !credits.total.is_finite() || credits.total <= 0.0 {
            return None;
        }
        Some(((credits.remaining / credits.total).clamp(0.0, 1.0) * 100.0).round() as u8)
    };
    [
        window_remaining(&snapshot.codex),
        credit_remaining(&snapshot.workbuddy),
        credit_remaining(&snapshot.trae),
        window_remaining(&snapshot.atomcode),
    ]
}

/// Whether the Windows taskbar uses the light theme. Read fresh on every
/// refresh so a theme switch is picked up with the next icon redraw.
#[cfg(windows)]
fn taskbar_is_dark() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD,
    };
    let mut value = 1_u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    // Missing key or read failure: assume the (more common) dark taskbar.
    status.is_ok() && value == 0
}

#[cfg(not(windows))]
fn taskbar_is_dark() -> bool {
    true
}

fn quota_image(snapshot: &UsageSnapshot) -> tauri::image::Image<'static> {
    tauri::image::Image::new_owned(
        render_quota_icon(quota_levels(snapshot), taskbar_is_dark()),
        ICON_SIZE,
        ICON_SIZE,
    )
}

pub(crate) fn update(app: &tauri::AppHandle, snapshot: &UsageSnapshot) {
    if let Some(tray) = app.tray_by_id("usage-tray") {
        let _ = tray.set_icon(Some(quota_image(snapshot)));
        let _ = tray.set_tooltip(Some(tray_tooltip(snapshot)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Credits, ProviderUsage, UsageWindow};

    #[test]
    fn quota_order_matches_providers_and_prefers_codex_five_hour_window() {
        let snapshot = UsageSnapshot {
            codex: ProviderUsage {
                status: "connected".into(),
                windows: vec![
                    UsageWindow {
                        label: "每周".into(),
                        used_percent: 90,
                        ..Default::default()
                    },
                    UsageWindow {
                        label: "5 小时".into(),
                        used_percent: 18,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            workbuddy: ProviderUsage {
                status: "connected".into(),
                credits: Some(Credits {
                    remaining: 25.0,
                    total: 100.0,
                    ..Default::default()
                }),
                ..Default::default()
            },
            trae: ProviderUsage {
                status: "connected".into(),
                credits: Some(Credits {
                    remaining: 3.0,
                    total: 10.0,
                    ..Default::default()
                }),
                ..Default::default()
            },
            atomcode: ProviderUsage {
                status: "connected".into(),
                windows: vec![UsageWindow {
                    used_percent: 65,
                    ..Default::default()
                }],
                ..Default::default()
            },
        };
        assert_eq!(
            quota_levels(&snapshot),
            [Some(82), Some(25), Some(30), Some(35)]
        );
    }

    #[test]
    fn unknown_and_invalid_credits_do_not_look_like_full_batteries() {
        let mut snapshot = crate::model::initial();
        snapshot.codex.windows.push(UsageWindow {
            used_percent: 0,
            ..Default::default()
        });
        snapshot.workbuddy.status = "connected".into();
        snapshot.workbuddy.credits = Some(Credits {
            remaining: 50.0,
            total: 0.0,
            ..Default::default()
        });
        snapshot.trae.status = "connected".into();
        snapshot.trae.credits = Some(Credits {
            remaining: f64::NAN,
            total: 100.0,
            ..Default::default()
        });
        assert_eq!(quota_levels(&snapshot), [None; 4]);
        snapshot.workbuddy.credits.as_mut().unwrap().total = 40.0;
        snapshot.trae.credits.as_mut().unwrap().remaining = -10.0;
        assert_eq!(quota_levels(&snapshot), [None, Some(100), Some(0), None]);
    }
}

pub(crate) fn tray_tooltip(snapshot: &UsageSnapshot) -> String {
    let mut tooltip = String::from("Win CodexBar · 剩余额度（顶部起顺时针）");
    for ((name, usage), level) in [
        ("Codex", &snapshot.codex),
        ("WorkBuddy", &snapshot.workbuddy),
        ("TRAE", &snapshot.trae),
        ("AtomCode", &snapshot.atomcode),
    ]
    .into_iter()
    .zip(quota_levels(snapshot))
    {
        let value = level
            .map(|percent| format!("{percent}%"))
            .unwrap_or_else(|| {
                match usage.status.as_str() {
                    "error" => "同步异常",
                    "connected" => "暂无额度数据",
                    _ => "未连接",
                }
                .into()
            });
        tooltip.push_str(&format!("\n{name}  {value}"));
    }
    tooltip
}

pub(crate) fn initialize(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let open = MenuItem::with_id(app, "open", "打开用量面板", true, None::<&str>)?;
    let refresh_item = MenuItem::with_id(app, "refresh", "刷新用量", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &refresh_item, &quit])?;
    let initial = crate::model::initial();
    TrayIconBuilder::with_id("usage-tray")
        .icon(quota_image(&initial))
        .tooltip(tray_tooltip(&initial))
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
