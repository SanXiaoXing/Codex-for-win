use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{
    window::{Color, Effect, EffectsBuilder},
    AppHandle, Manager, State,
};
/// 进入悬浮球模式前保存的面板位置（物理像素），退出时据此恢复。
pub(crate) struct SavedPosition(pub(crate) Mutex<Option<tauri::PhysicalPosition<i32>>>);

/// 悬浮球窗口的逻辑边长与贴边留白（物理像素 = 逻辑值 × 显示器缩放比）。
/// 该值与前端 `.ball-circle`（填满整个窗口）严格一致，避免窗口里出现未被球填满的死区：
/// 死区会被 DWM 的方形拖动阴影/拖动指示器显形，拖动后看起来像球"展开"了。
const BALL_LOGICAL: f64 = 60.0;
const BALL_EDGE_GAP: f64 = 4.0;

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
pub(crate) async fn set_ball_mode(
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

        // 给 Windows 一个明确的悬浮球最小尺寸，避免原面板的最小尺寸约束
        // 继续生效后只裁成椭圆，却仍保留整块面板大小。
        window
            .set_min_size(Some(tauri::LogicalSize::new(BALL_LOGICAL, BALL_LOGICAL)))
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
            Some("left") => (mx + gap, (center_y - ball / 2).clamp(my, my + mh - ball)),
            Some("right") => (
                mx + mw - ball - gap,
                (center_y - ball / 2).clamp(my, my + mh - ball),
            ),
            // 默认吸附到顶部边缘，保留窗口当前的横向位置
            _ => ((center_x - ball / 2).clamp(mx, mx + mw - ball), my + gap),
        };
        morph_window(
            window.clone(),
            tauri::PhysicalPosition::new(x, y),
            tauri::PhysicalSize::new(ball as u32, ball as u32),
            true,
            reduced_motion.unwrap_or(false),
        )
        .await?;
        // Native frame animation uses physical bounds; finish with an explicit logical
        // size so per-monitor DPI and WM_MINMAXINFO cannot leave the original panel size.
        window
            .set_size(tauri::LogicalSize::new(BALL_LOGICAL, BALL_LOGICAL))
            .map_err(err)?;
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
                            (x).clamp(mx, mx + mw - width),
                            (y).clamp(my, my + mh - height),
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
        window
            .set_size(tauri::LogicalSize::new(368.0, 560.0))
            .map_err(err)?;
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

pub(crate) fn configure_main_window(app: &tauri::App) {
    if let Some(window) = app.get_webview_window("main") {
        #[cfg(target_os = "windows")]
        {
            let observed = window.clone();
            window.on_window_event(move |event| {
                if matches!(
                    event,
                    tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. }
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
}
