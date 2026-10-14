use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use keyring::Entry;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    window::{Color, Effect, EffectsBuilder},
    AppHandle, Emitter, Manager, State,
};

const SERVICE: &str = "codexbar-windows-mvp";
const WORKBUDDY_COOKIE_KEY: &str = "workbuddy-cookie";
const WORKBUDDY_UA_KEY: &str = "workbuddy-user-agent";
const CODEX_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const WORKBUDDY_URL: &str = "https://www.workbuddy.cn/billing/meter/get-user-resource-summary";

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct UsageWindow {
    label: String,
    used_percent: u8,
    resets_at: Option<i64>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Credits {
    remaining: f64,
    total: f64,
    unit: String,
    plan: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ProviderUsage {
    provider: String,
    status: String,
    account: Option<String>,
    windows: Vec<UsageWindow>,
    credits: Option<Credits>,
    updated_at: Option<i64>,
    error: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct UsageSnapshot {
    codex: ProviderUsage,
    workbuddy: ProviderUsage,
    refreshed_at: Option<i64>,
}

struct UsageState(Mutex<UsageSnapshot>);

/// 进入悬浮球模式前保存的面板位置（物理像素），退出时据此恢复。
struct SavedPosition(Mutex<Option<tauri::PhysicalPosition<i32>>>);

/// 悬浮球窗口的逻辑边长与贴边留白（物理像素 = 逻辑值 × 显示器缩放比）。
/// 该值与前端 `.ball-circle`（填满整个窗口）严格一致，避免窗口里出现未被球填满的死区：
/// 死区会被 DWM 的方形拖动阴影/拖动指示器显形，拖动后看起来像球"展开"了。
const BALL_LOGICAL: f64 = 60.0;
const BALL_EDGE_GAP: f64 = 4.0;

fn clamp_i32(value: i32, low: i32, high: i32) -> i32 {
    value.max(low).min(high.max(low))
}

/// Win11：让无装饰透明窗口的 DWM 合成面圆角与 CSS `--window-radius` 对齐，
/// 否则 Acrylic 背衬会以方角铺满，圆角处露出方形亚克力边缘。
/// Win10 / 旧版本调用返回错误 HRESULT，忽略即可（仅 Win11 22000+ 支持）。
#[cfg(target_os = "windows")]
fn prefer_round_corners(window: &tauri::WebviewWindow) {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
    };
    const DWMWCP_ROUND: i32 = 2;
    const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;
    if let Ok(hwnd) = window.hwnd() {
        let mut value = DWMWCP_ROUND;
        let mut border = DWMWA_COLOR_NONE;
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &mut value as *mut i32 as *mut std::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &mut border as *mut u32 as *mut std::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}

/// CSS 圆角只裁 WebView 内容；窗口区域也必须同步裁成圆形，
/// 否则 Windows 仍会显示 72×72 的矩形边框/背景。
#[cfg(target_os = "windows")]
fn set_ball_region(window: &tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    use windows::Win32::Graphics::Gdi::{
        CreateEllipticRgn, CreateRectRgn, DeleteObject, EqualRgn, GetWindowRgn, SetWindowRgn,
        HGDIOBJ,
    };
    use windows::Win32::{Foundation::RECT, UI::WindowsAndMessaging::GetWindowRect};

    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    unsafe {
        if enabled {
            // Read the HWND bounds directly. Tauri's cached size can lag WM_SIZE during
            // DPI transitions, leaving small rectangular strips outside the circle.
            let mut bounds = RECT::default();
            GetWindowRect(hwnd, &mut bounds).map_err(|e| e.to_string())?;
            let width = bounds.right - bounds.left;
            let height = bounds.bottom - bounds.top;
            let region = CreateEllipticRgn(0, 0, width, height);
            if region.is_invalid() {
                return Err("无法创建悬浮球窗口区域".into());
            }
            // SetWindowRgn 自身也会触发窗口事件，已有相同区域时不要再次设置。
            let current = CreateRectRgn(0, 0, 0, 0);
            if !current.is_invalid() {
                let same =
                    GetWindowRgn(hwnd, current).0 != 0 && EqualRgn(current, region).as_bool();
                let _ = DeleteObject(HGDIOBJ(current.0));
                if same {
                    let _ = DeleteObject(HGDIOBJ(region.0));
                    return Ok(());
                }
            }
            if SetWindowRgn(hwnd, Some(region), true) == 0 {
                let _ = DeleteObject(HGDIOBJ(region.0));
                return Err("无法设置悬浮球窗口区域".into());
            }
        } else if SetWindowRgn(hwnd, None, true) == 0 {
            return Err("无法恢复面板窗口区域".into());
        }
    }
    Ok(())
}

/// 原生窗口使用 drawer 曲线：cubic-bezier(.32, .72, 0, 1)。
#[cfg(target_os = "windows")]
fn drawer_progress(progress: f64) -> f64 {
    if progress <= 0.0 || progress >= 1.0 {
        return progress.clamp(0.0, 1.0);
    }
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..16 {
        let t = (low + high) / 2.0;
        let x = 3.0 * (1.0 - t) * (1.0 - t) * t * 0.32 + t * t * t;
        if x < progress {
            low = t;
        } else {
            high = t;
        }
    }
    let t = (low + high) / 2.0;
    3.0 * (1.0 - t) * (1.0 - t) * t * 0.72 + 3.0 * (1.0 - t) * t * t + t * t * t
}

/// 位置和尺寸在同一个原生调用中更新；圆角区域同步变化，避免矩形底衬闪现。
/// 在工作线程等待帧间隔，主线程继续处理 WebView 绘制和窗口消息。
async fn morph_window(
    window: tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
    to_ball: bool,
    reduced_motion: bool,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
            use std::time::{Duration, Instant};
            use windows::Win32::Graphics::Gdi::{
                CreateRoundRectRgn, DeleteObject, SetWindowRgn, HGDIOBJ,
            };
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER,
            };

            let hwnd = window.hwnd().map_err(|e| e.to_string())?;
            let start_position = window.outer_position().map_err(|e| e.to_string())?;
            let start_size = window.outer_size().map_err(|e| e.to_string())?;
            let scale = window.scale_factor().map_err(|e| e.to_string())?;
            let (start_radius, end_radius) = if to_ball { (12.0, 30.0) } else { (30.0, 12.0) };
            let duration = Duration::from_millis(if reduced_motion {
                0
            } else if to_ball {
                240
            } else {
                280
            });
            let started = Instant::now();
            loop {
                let progress = if duration.is_zero() {
                    1.0
                } else {
                    (started.elapsed().as_secs_f64() / duration.as_secs_f64()).min(1.0)
                };
                let eased = drawer_progress(progress);
                let lerp = |from: f64, to: f64| (from + (to - from) * eased).round() as i32;
                let width = lerp(start_size.width as f64, size.width as f64);
                let height = lerp(start_size.height as f64, size.height as f64);
                let diameter = lerp(start_radius * scale * 2.0, end_radius * scale * 2.0);
                unsafe {
                    SetWindowPos(
                        hwnd,
                        None,
                        lerp(start_position.x as f64, position.x as f64),
                        lerp(start_position.y as f64, position.y as f64),
                        width,
                        height,
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    )
                    .map_err(|e| e.to_string())?;
                    let region =
                        CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter);
                    if region.is_invalid() {
                        return Err("无法创建过渡窗口区域".into());
                    }
                    if SetWindowRgn(hwnd, Some(region), true) == 0 {
                        let _ = DeleteObject(HGDIOBJ(region.0));
                        return Err("无法更新过渡窗口区域".into());
                    }
                }
                if progress >= 1.0 {
                    break;
                }
                std::thread::sleep(
                    Duration::from_millis(16).min(duration.saturating_sub(started.elapsed())),
                );
            }
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (to_ball, reduced_motion);
        window.set_size(size).map_err(|e| e.to_string())?;
        window.set_position(position).map_err(|e| e.to_string())
    }
}

/// 吸边悬浮球模式：`enabled=true` 时窗口缩为贴边小球并置顶；
/// `enabled=false` 时恢复面板。`side` 为吸附边（left/right/top），
/// `placement` 为恢复位置策略（saved=回到吸附前位置，here=以球当前位置就地展开）。
#[tauri::command]
async fn set_ball_mode(
    app: AppHandle,
    saved: State<'_, SavedPosition>,
    enabled: bool,
    side: Option<String>,
    placement: Option<String>,
    reduced_motion: Option<bool>,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;
    let err = |e: tauri::Error| e.to_string();

    if enabled {
        let position = window.outer_position().map_err(err)?;
        // 面板不可手动缩放，展开时始终使用固定尺寸，只需保存吸附前的位置。
        let size = window.outer_size().map_err(err)?;
        *saved.0.lock().unwrap() = Some(position);

        // 面板的最小尺寸会阻止 Windows 将窗口缩至悬浮球大小。
        window
            .set_min_size(None::<tauri::LogicalSize<f64>>)
            .map_err(err)?;
        // 动画期间关闭自动椭圆裁切，逐帧应用圆角矩形区域。
        app.state::<AtomicBool>().store(false, Ordering::SeqCst);

        window.set_always_on_top(true).map_err(err)?;
        window.set_skip_taskbar(true).map_err(err)?;
        // 关闭系统方形阴影，避免圆球外露方角阴影残影
        let _ = window.set_shadow(false);
        // 球形窗口移除亚克力底衬，避免圆形外露出方形模糊区域
        let _ = window.set_effects(None);

        let monitor = window
            .current_monitor()
            .map_err(err)?
            .ok_or_else(|| "无法获取当前显示器".to_string())?;
        let scale = window.scale_factor().map_err(err)? as f64;
        let ball = (BALL_LOGICAL * scale).round() as i32;
        let gap = (BALL_EDGE_GAP * scale).round() as i32;
        let (mx, my) = (monitor.position().x, monitor.position().y);
        let (mw, mh) = (monitor.size().width as i32, monitor.size().height as i32);
        let center_x = position.x + size.width as i32 / 2;
        let center_y = position.y + size.height as i32 / 2;
        let (x, y) = match side.as_deref() {
            Some("left") => (mx + gap, clamp_i32(center_y - ball / 2, my, my + mh - ball)),
            Some("right") => (
                mx + mw - ball - gap,
                clamp_i32(center_y - ball / 2, my, my + mh - ball),
            ),
            // 默认吸附到顶部边缘，保留窗口当前的横向位置
            _ => (clamp_i32(center_x - ball / 2, mx, mx + mw - ball), my + gap),
        };
        morph_window(
            window.clone(),
            tauri::PhysicalPosition::new(x, y),
            tauri::PhysicalSize::new(ball as u32, ball as u32),
            true,
            reduced_motion.unwrap_or(false),
        )
        .await?;
        app.state::<AtomicBool>().store(true, Ordering::SeqCst);
        #[cfg(target_os = "windows")]
        set_ball_region(&window, true)?;
    } else {
        app.state::<AtomicBool>().store(false, Ordering::SeqCst);
        let scale = window.scale_factor().map_err(err)? as f64;
        let saved_position = *saved.0.lock().unwrap();
        let size = tauri::PhysicalSize::new(
            (368.0 * scale).round() as u32,
            (560.0 * scale).round() as u32,
        );
        let position = match placement.as_deref() {
            // 就地展开：以悬浮球当前位置为锚点，整体夹回显示器可视区内
            Some("here") => {
                let current = window.outer_position().map_err(err)?;
                match window.current_monitor().map_err(err)? {
                    Some(monitor) => {
                        let (mx, my) = (monitor.position().x, monitor.position().y);
                        let (mw, mh) = (monitor.size().width as i32, monitor.size().height as i32);
                        let ball = (BALL_LOGICAL * scale).round() as i32;
                        let width = size.width as i32;
                        let height = size.height as i32;
                        let (x, y) = match side.as_deref() {
                            Some("left") => (current.x, current.y + (ball - height) / 2),
                            Some("top") => (current.x + (ball - width) / 2, current.y),
                            _ => (current.x + ball - width, current.y + (ball - height) / 2),
                        };
                        tauri::PhysicalPosition::new(
                            clamp_i32(x, mx, mx + mw - width),
                            clamp_i32(y, my, my + mh - height),
                        )
                    }
                    None => saved_position.unwrap_or(current),
                }
            }
            _ => match saved_position {
                Some(g) => g,
                None => {
                    window.set_size(size).map_err(err)?;
                    window.center().map_err(err)?;
                    window.outer_position().map_err(err)?
                }
            },
        };
        morph_window(
            window.clone(),
            position,
            size,
            false,
            reduced_motion.unwrap_or(false),
        )
        .await?;
        #[cfg(target_os = "windows")]
        set_ball_region(&window, false)?;
        window
            .set_min_size(Some(tauri::LogicalSize::new(350.0, 480.0)))
            .map_err(err)?;
        window.set_always_on_top(false).map_err(err)?;
        window.set_skip_taskbar(false).map_err(err)?;
        // Tauri 的 Windows 无装饰阴影会补一圈系统边框，和透明 Acrylic 面板重叠后
        // 看起来像原生标题栏/蓝色外框；面板层已有 CSS 阴影，不再恢复系统阴影。
        let _ = window.set_shadow(false);
        let _ = window.set_effects(
            EffectsBuilder::new()
                .effect(Effect::Acrylic)
                .color(Color(238, 241, 255, 92))
                .build(),
        );
        #[cfg(target_os = "windows")]
        prefer_round_corners(&window);
    }
    Ok(())
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn tray_tooltip(snapshot: &UsageSnapshot) -> String {
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
    if codex.is_empty() {
        format!("CodexBar · Codex 未连接 · {workbuddy}")
    } else {
        format!("CodexBar · {} · {workbuddy}", codex.join(" / "))
    }
}

fn initial() -> UsageSnapshot {
    UsageSnapshot {
        codex: ProviderUsage {
            provider: "codex".into(),
            status: "needs_login".into(),
            ..Default::default()
        },
        workbuddy: ProviderUsage {
            provider: "workbuddy".into(),
            status: "needs_login".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn codex_auth_path() -> PathBuf {
    if let Some(home) = std::env::var_os("CODEX_HOME") {
        return PathBuf::from(home).join("auth.json");
    }
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex")
        .join("auth.json")
}

fn auth_token_and_account() -> Result<(String, Option<String>), String> {
    let path = codex_auth_path();
    let contents = fs::read_to_string(&path).map_err(|_| {
        format!(
            "找不到 Codex 登录文件：{}。请先登录 Codex CLI。",
            path.display()
        )
    })?;
    let auth: Value =
        serde_json::from_str(&contents).map_err(|_| "Codex 登录文件格式无效".to_string())?;
    let tokens = auth.get("tokens").unwrap_or(&auth);
    let token = tokens
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "Codex 登录文件中没有 access token；请重新登录 Codex CLI。".to_string())?
        .to_string();
    let account = tokens
        .get("account_id")
        .or_else(|| auth.get("account_id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| account_from_jwt(&token));
    Ok((token, account))
}

fn account_from_jwt(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: Value = serde_json::from_slice(&decoded).ok()?;
    claims
        .get("https://api.openai.com/auth")?
        .get("chatgpt_account_id")?
        .as_str()
        .map(str::to_owned)
}

fn reset_at(window: &Value) -> Option<i64> {
    window.get("reset_at").and_then(Value::as_i64).or_else(|| {
        window
            .get("reset_at")
            .and_then(Value::as_f64)
            .map(|n| n as i64)
    })
}

async fn fetch_codex(client: &Client) -> ProviderUsage {
    let result = async {
        let (token, account) = auth_token_and_account()?;
        let mut request = client
            .get(CODEX_URL)
            .bearer_auth(token)
            .header(header::USER_AGENT, "codexbar-windows-mvp/0.1");
        if let Some(id) = &account {
            request = request.header("ChatGPT-Account-Id", id);
        }
        let response = request
            .send()
            .await
            .map_err(|e| format!("连接 Codex 用量接口失败：{e}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Codex 用量接口返回 HTTP {}；可尝试重新登录 Codex CLI。",
                response.status()
            ));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|_| "Codex 用量响应无法解析".to_string())?;
        let rate = body.get("rate_limit").unwrap_or(&body);
        let mut windows = Vec::new();
        for (key, label) in [("primary_window", "5 小时"), ("secondary_window", "每周")] {
            if let Some(window) = rate.get(key) {
                if let Some(value) = window.get("used_percent").and_then(Value::as_f64) {
                    windows.push(UsageWindow {
                        label: label.into(),
                        used_percent: value.round().clamp(0.0, 100.0) as u8,
                        resets_at: reset_at(window),
                    });
                }
            }
        }
        if windows.is_empty() {
            return Err("Codex 未返回可识别的用量窗口".to_string());
        }
        let account_name = body.get("email").and_then(Value::as_str).map(str::to_owned);
        Ok(ProviderUsage {
            provider: "codex".into(),
            status: "connected".into(),
            account: account_name,
            windows,
            credits: None,
            updated_at: Some(now()),
            error: None,
        })
    }
    .await;
    provider_or_error("codex", result, |error| {
        error.contains("找不到 Codex 登录文件") || error.contains("没有 access token")
    })
}

/// Windows 凭据库单条凭据上限约 2560 字符（UTF-16），WorkBuddy 的整串 Cookie
/// 通常超过该限制，因此按块拆分为多条凭据存储：`{key}-part0..N-1` + `{key}-count`。
const SECRET_CHUNK_SIZE: usize = 1000;

fn split_secret<'a>(value: &'a str, size: usize) -> Vec<&'a str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < value.len() {
        let mut end = (start + size).min(value.len());
        while end > start && !value.is_char_boundary(end) {
            end -= 1;
        }
        chunks.push(&value[start..end]);
        start = end;
    }
    chunks
}

fn stored_part_count(key: &str) -> usize {
    Entry::new(SERVICE, &format!("{key}-count"))
        .and_then(|entry| entry.get_password())
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
}

fn delete_secret(key: &str) {
    if let Ok(entry) = Entry::new(SERVICE, key) {
        let _ = entry.delete_credential();
    }
    for index in 0..stored_part_count(key) {
        if let Ok(entry) = Entry::new(SERVICE, &format!("{key}-part{index}")) {
            let _ = entry.delete_credential();
        }
    }
    if let Ok(entry) = Entry::new(SERVICE, &format!("{key}-count")) {
        let _ = entry.delete_credential();
    }
}

fn write_secret(key: &str, value: &str) -> Result<(), String> {
    delete_secret(key);
    let chunks = split_secret(value, SECRET_CHUNK_SIZE);
    for (index, chunk) in chunks.iter().enumerate() {
        Entry::new(SERVICE, &format!("{key}-part{index}"))
            .map_err(|e| e.to_string())?
            .set_password(chunk)
            .map_err(|e| e.to_string())?;
    }
    Entry::new(SERVICE, &format!("{key}-count"))
        .map_err(|e| e.to_string())?
        .set_password(&chunks.len().to_string())
        .map_err(|e| e.to_string())
}

fn read_secret(key: &str) -> Result<String, String> {
    let count = stored_part_count(key);
    if count == 0 {
        return Err(format!("凭据 {key} 不存在或为空"));
    }
    let mut parts = Vec::with_capacity(count);
    for index in 0..count {
        parts.push(
            Entry::new(SERVICE, &format!("{key}-part{index}"))
                .map_err(|e| e.to_string())?
                .get_password()
                .map_err(|e| e.to_string())?,
        );
    }
    Ok(parts.concat())
}

#[cfg(test)]
mod secret_tests {
    use super::split_secret;

    #[test]
    fn split_roundtrip_keeps_content() {
        let value = "a".repeat(2500) + "中文尾部";
        let joined: String = split_secret(&value, 1000).concat();
        assert_eq!(joined, value);
        assert!(split_secret(&value, 1000).iter().all(|c| c.len() <= 1000));
    }

    #[test]
    fn split_short_value_single_chunk() {
        assert_eq!(split_secret("abc", 1000), vec!["abc"]);
    }
}

async fn fetch_workbuddy(client: &Client) -> ProviderUsage {
    let result = async {
        let cookie = read_secret(WORKBUDDY_COOKIE_KEY)
            .map_err(|_| "请先在设置中添加 WorkBuddy 浏览器会话".to_string())?;
        let ua = read_secret(WORKBUDDY_UA_KEY)
            .map_err(|_| "请先在设置中添加 WorkBuddy 浏览器 User-Agent".to_string())?;
        let response = client
            .post(WORKBUDDY_URL)
            .header(header::COOKIE, cookie)
            .header(header::USER_AGENT, ua)
            .json(&json!({}))
            .send()
            .await
            .map_err(|e| format!("连接 WorkBuddy 用量接口失败：{e}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "WorkBuddy 返回 HTTP {}；Cookie 与 User-Agent 必须来自同一个浏览器请求。",
                response.status()
            ));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|_| "WorkBuddy 用量响应无法解析".to_string())?;
        let packages = body
            .pointer("/data/Packages")
            .and_then(Value::as_array)
            .ok_or_else(|| "WorkBuddy 响应中没有积分套餐".to_string())?;
        let mut total = 0.0;
        let mut remaining = 0.0;
        let mut plan: Option<String> = None;
        for package in packages {
            if package
                .get("CapacityUnit")
                .and_then(Value::as_str)
                .map(|s| s.eq_ignore_ascii_case("credits"))
                .unwrap_or(false)
            {
                total += number(package.get("CycleTotalCapacity"));
                remaining += number(package.get("CycleRemainCapacity"));
                if plan.is_none() {
                    plan = package
                        .get("SubscriptionPackageName")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                }
            }
        }
        if total <= 0.0 {
            return Err("WorkBuddy 账户中没有可显示的积分套餐".to_string());
        }
        Ok(ProviderUsage {
            provider: "workbuddy".into(),
            status: "connected".into(),
            account: None,
            windows: Vec::new(),
            credits: Some(Credits {
                remaining,
                total,
                unit: "积分".into(),
                plan,
            }),
            updated_at: Some(now()),
            error: None,
        })
    }
    .await;
    provider_or_error("workbuddy", result, |error| error.contains("请先在设置"))
}

fn number(value: Option<&Value>) -> f64 {
    value
        .and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
        })
        .unwrap_or(0.0)
}

/// 两个 fetch 共用的错误兜底：Ok 直接返回；Err 时按 `needs_login_pred`
/// 把错误归类为 `needs_login` 或 `error`，并填入 provider/错误信息。
fn provider_or_error(
    provider: &str,
    result: Result<ProviderUsage, String>,
    needs_login_pred: impl Fn(&str) -> bool,
) -> ProviderUsage {
    result.unwrap_or_else(|error| ProviderUsage {
        provider: provider.into(),
        status: if needs_login_pred(&error) {
            "needs_login"
        } else {
            "error"
        }
        .into(),
        error: Some(error),
        ..Default::default()
    })
}

async fn refresh(app: &AppHandle) -> UsageSnapshot {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(18))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let host = attempt.url().host_str().unwrap_or_default();
            let allowed = host == "chatgpt.com"
                || host.ends_with(".chatgpt.com")
                || host == "workbuddy.cn"
                || host.ends_with(".workbuddy.cn");
            if allowed {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .unwrap_or_else(|_| Client::new());
    let (codex, workbuddy) = tokio::join!(fetch_codex(&client), fetch_workbuddy(&client));
    let snapshot = UsageSnapshot {
        codex,
        workbuddy,
        refreshed_at: Some(now()),
    };
    if let Some(state) = app.try_state::<UsageState>() {
        if let Ok(mut current) = state.0.lock() {
            *current = snapshot.clone();
        }
    }
    if let Some(tray) = app.tray_by_id("usage-tray") {
        let _ = tray.set_tooltip(Some(tray_tooltip(&snapshot)));
    }
    let _ = app.emit("usage-updated", snapshot.clone());
    snapshot
}

#[tauri::command]
fn get_usage(state: State<'_, UsageState>) -> UsageSnapshot {
    state
        .0
        .lock()
        .map(|s| s.clone())
        .unwrap_or_else(|_| initial())
}

#[tauri::command]
async fn refresh_usage(app: AppHandle) -> UsageSnapshot {
    refresh(&app).await
}

#[tauri::command]
fn save_workbuddy_session(cookie: String, user_agent: String) -> Result<(), String> {
    if cookie.is_empty() || user_agent.is_empty() {
        return Err("Cookie 和 User-Agent 都不能为空".into());
    }
    write_secret(WORKBUDDY_COOKIE_KEY, &cookie)?;
    write_secret(WORKBUDDY_UA_KEY, &user_agent)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .manage(UsageState(Mutex::new(initial())))
        .manage(SavedPosition(Mutex::new(None)))
        .manage(AtomicBool::new(false))
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "windows")]
                {
                    let observed = window.clone();
                    window.on_window_event(move |event| {
                        if matches!(
                            event,
                            tauri::WindowEvent::Resized(_)
                                | tauri::WindowEvent::ScaleFactorChanged { .. }
                        ) && observed.state::<AtomicBool>().load(Ordering::SeqCst)
                        {
                            if let Err(error) = set_ball_region(&observed, true) {
                                eprintln!("Could not update the floating ball region: {error}");
                            }
                        }
                    });
                }
                if let Err(error) = window.set_effects(
                    EffectsBuilder::new()
                        .effect(Effect::Acrylic)
                        .color(Color(238, 241, 255, 92))
                        .build(),
                ) {
                    eprintln!("Could not enable the Windows acrylic backdrop: {error}");
                }
                #[cfg(target_os = "windows")]
                prefer_round_corners(&window);
            }
            let open = MenuItem::with_id(app, "open", "打开用量面板", true, None::<&str>)?;
            let refresh_item = MenuItem::with_id(app, "refresh", "刷新用量", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &refresh_item, &quit])?;
            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;
            TrayIconBuilder::with_id("usage-tray")
                .icon(icon)
                .tooltip("CodexBar · AI 用量")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            // 悬浮球模式下让前端恢复成完整面板
                            let _ = app.emit("restore-panel", ());
                        }
                    }
                    "refresh" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            refresh(&app).await;
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
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = tray.app_handle().emit("restore-panel", ());
                        }
                    }
                })
                .build(app)?;
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    refresh(&app_handle).await;
                    tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_usage,
            refresh_usage,
            save_workbuddy_session,
            set_ball_mode
        ])
        .run(tauri::generate_context!())
        .expect("error while running CodexBar");
}
