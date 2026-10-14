use crate::{
    credentials::{
        write_secret, TRAE_AUTH_KEY, TRAE_DEVICE_ID_KEY, WORKBUDDY_COOKIE_KEY, WORKBUDDY_UA_KEY,
    },
    model::{initial, UsageSnapshot, UsageState},
    providers,
};
use tauri::{AppHandle, State};

pub(crate) mod trae_checkin;

#[tauri::command]
pub(crate) fn get_usage(state: State<'_, UsageState>) -> UsageSnapshot {
    state
        .0
        .lock()
        .map(|s| s.clone())
        .unwrap_or_else(|_| initial())
}

#[tauri::command]
pub(crate) async fn refresh_usage(app: AppHandle) -> UsageSnapshot {
    providers::refresh(&app).await
}

#[tauri::command]
pub(crate) fn save_workbuddy_session(cookie: String, user_agent: String) -> Result<(), String> {
    if cookie.is_empty() || user_agent.is_empty() {
        return Err("Cookie 和 User-Agent 都不能为空".into());
    }
    write_secret(WORKBUDDY_COOKIE_KEY, &cookie)?;
    write_secret(WORKBUDDY_UA_KEY, &user_agent)?;
    Ok(())
}

#[tauri::command]
pub(crate) fn save_trae_authorization(
    authorization: String,
    device_id: String,
) -> Result<(), String> {
    let value = authorization.trim();
    if value.is_empty() {
        return Err("Authorization 不能为空".into());
    }
    if !value.starts_with("Cloud-IDE-JWT ") && !value.starts_with("Bearer ") {
        return Err("请粘贴完整 Authorization 值（Cloud-IDE-JWT 或 Bearer）".into());
    }
    write_secret(TRAE_AUTH_KEY, value)?;
    write_secret(TRAE_DEVICE_ID_KEY, device_id.trim())
}

#[tauri::command]
pub(crate) fn open_trae_dashboard() -> Result<(), String> {
    let url = "https://www.trae.cn/dashboard#usage";
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    result.map(|_| ()).map_err(|error| error.to_string())
}
