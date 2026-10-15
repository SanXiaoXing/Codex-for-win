use crate::model::{ProviderUsage, UsageSnapshot, UsageState};
use crate::tray::tray_tooltip;
use reqwest::Client;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

mod codex;
pub(crate) mod trae;
mod workbuddy;
use codex::fetch_codex;
use trae::fetch_trae;
use workbuddy::fetch_workbuddy;

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub(crate) fn number(value: Option<&Value>) -> f64 {
    value
        .and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
        })
        .unwrap_or(0.0)
}

pub(crate) fn safe_error_body(body: &str, secrets: &[&str]) -> String {
    let redacted = secrets
        .iter()
        .filter(|secret| !secret.is_empty())
        .fold(body.to_owned(), |text, secret| {
            text.replace(secret, "[已隐藏]")
        });
    let compact = redacted.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return "响应正文为空".into();
    }
    compact.chars().take(240).collect()
}

/// 两个 fetch 共用的错误兜底：Ok 直接返回；Err 时按 `needs_login_pred`
/// 把错误归类为 `needs_login` 或 `error`，并填入 provider/错误信息。
pub(crate) fn provider_or_error(
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

pub(crate) async fn refresh(app: &AppHandle) -> UsageSnapshot {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(18))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let host = attempt.url().host_str().unwrap_or_default();
            let allowed = host == "chatgpt.com"
                || host.ends_with(".chatgpt.com")
                || host == "workbuddy.cn"
                || host.ends_with(".workbuddy.cn")
                || host == "trae.cn"
                || host.ends_with(".trae.cn");
            if allowed {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .unwrap_or_else(|_| Client::new());
    let (codex, workbuddy, trae) = tokio::join!(
        fetch_codex(&client),
        fetch_workbuddy(&client),
        fetch_trae(&client)
    );
    let snapshot = UsageSnapshot {
        codex,
        workbuddy,
        trae,
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
