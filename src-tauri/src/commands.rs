use crate::{
    credentials::{
        delete_secret, write_secret, TRAE_AUTH_KEY, TRAE_COOKIE_KEY, TRAE_UA_KEY,
        WORKBUDDY_COOKIE_KEY, WORKBUDDY_UA_KEY,
    },
    model::{initial, UsageSnapshot, UsageState},
    providers,
};
use tauri::{AppHandle, State};

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

/// 保存 TRAE 浏览器会话 Cookie（推荐）：CodexBar 用它定时换取 8 小时 JWT，实现自动续期。
#[tauri::command]
pub(crate) fn save_trae_session(cookie: String, user_agent: String) -> Result<(), String> {
    if cookie.trim().is_empty() {
        return Err("Cookie 不能为空".into());
    }
    if !cookie.contains('=') {
        return Err("请粘贴完整 Cookie 内容（形如 a=b; c=d）".into());
    }
    write_secret(TRAE_COOKIE_KEY, cookie.trim())?;
    if user_agent.trim().is_empty() {
        // User-Agent 留空时清除旧值，查询时回落到内置浏览器 UA。
        delete_secret(TRAE_UA_KEY);
    } else {
        write_secret(TRAE_UA_KEY, user_agent.trim())?;
    }
    Ok(())
}

/// 保存 TRAE 浏览器会话（旧的 Authorization 直填方式，作为备用）。
#[tauri::command]
pub(crate) fn save_trae_authorization(authorization: String) -> Result<(), String> {
    let value = authorization.trim();
    if value.is_empty() {
        return Err("Authorization 不能为空".into());
    }
    if !value.starts_with("Cloud-IDE-JWT ") && !value.starts_with("Bearer ") {
        return Err("请粘贴完整 Authorization 值（Cloud-IDE-JWT 或 Bearer）".into());
    }
    write_secret(TRAE_AUTH_KEY, value)
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
