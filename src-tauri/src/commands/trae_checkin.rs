use crate::credentials::{read_secret, write_secret, TRAE_CHECKIN_SESSION_KEY, TRAE_DEVICE_ID_KEY};

#[tauri::command]
pub(crate) fn save_trae_checkin_credentials(
    session: String,
    device_id: String,
) -> Result<(), String> {
    let value = session.trim();
    if value.is_empty() {
        return Err("TRAE 签到会话不能为空".into());
    }
    let session = value
        .split(';')
        .find_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            name.eq_ignore_ascii_case("X-Cloudide-Session")
                .then_some(value.trim())
        })
        .unwrap_or(value)
        .trim();
    if session.is_empty() {
        return Err("没有读取到 X-Cloudide-Session 值".into());
    }
    if device_id.trim().is_empty() {
        return Err("请填写 TRAE 请求中的 x-device-id".into());
    }
    write_secret(TRAE_CHECKIN_SESSION_KEY, session)?;
    write_secret(TRAE_DEVICE_ID_KEY, device_id.trim())
}

#[tauri::command]
pub(crate) async fn trae_checkin() -> Result<String, String> {
    let session = read_secret(TRAE_CHECKIN_SESSION_KEY)
        .map_err(|_| "请先在设置中保存 TRAE 签到会话 Cookie".to_string())?;
    let device_id = read_secret(TRAE_DEVICE_ID_KEY)
        .map_err(|_| "请先在设置中保存 TRAE x-device-id".to_string())?;
    crate::checkin::trae::check_in(&session, &device_id).await
}
