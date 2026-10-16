//! AtomCode（AtomGit CodingPlan）用量提供方。
//!
//! 与 Codex / WorkBuddy / TRAE 不同，AtomCode 不需要手工粘贴凭据：
//! CLI 会把登录态写在 `~/.atomcode/auth.toml`，本模块直接读取该文件的 `access_token`，
//! 再调用 CodingPlan 状态与明细接口，得到「滚动窗口用量 + 窗口重置时间 + 套餐有效期 + 近 60 天 Token/调用量」。
use crate::{
    model::{Credits, ProviderUsage, UsageMetric, UsageWindow},
    providers::{now, provider_or_error, safe_error_body},
};
use reqwest::{header, Client};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Mutex;

const ATOMCODE_BASE: &str = "https://api.atomgit.com/api/v5";
const ATOMCODE_STATUS_PATH: &str = "/coding-plan/status-v2";
const ATOMCODE_USAGE_PATH: &str = "/coding-plan/usage";
/// 每日签到（领取 CodingPlan 权益）接口。
const ATOMCODE_CLAIM_PATH: &str = "/coding-plan/claim-v2";
/// 成功签到后的静默期：一天一次即可，不要每 5 分钟都去打领取接口。
const CLAIM_INTERVAL_SECS: i64 = 20 * 3600;
/// 失败后的最小重试间隔，防止接口异常时被高频重试。
const CLAIM_RETRY_SECS: i64 = 600;

/// 最近一次签到尝试 (成功时间, 尝试时间)，用于决定本轮是否要签到。
static LAST_CLAIM: Mutex<(Option<i64>, Option<i64>)> = Mutex::new((None, None));

pub(crate) async fn fetch_atomcode(client: &Client) -> ProviderUsage {
    let result = async {
        let auth = read_auth()?;
        let status = get_json(client, ATOMCODE_STATUS_PATH, &auth).await?;
        // 明细接口偶尔会 401/超时，缺失时只少几行指标，不影响主数据展示。
        let usage = get_json(client, ATOMCODE_USAGE_PATH, &auth).await.ok();
        let mut parsed = parse_status(&status, usage.as_ref(), auth.username.as_deref())?;
        if let Some(note) = claim_daily(client, &auth, &status, usage.as_ref()).await {
            parsed.account = Some(match parsed.account {
                Some(account) => format!("{account} · {note}"),
                None => note,
            });
        }
        Ok(parsed)
    }
    .await;

    provider_or_error("atomcode", result, |error| {
        error.contains("未找到") || error.contains("重新登录")
    })
}

/// 每日签到：把 status-v2 给出的 `plan_type` 提交给 claim-v2 领取当日权益。
/// 返回一句可展示的结论；无需签到时返回 None。
async fn claim_daily(
    client: &Client,
    auth: &Auth,
    status: &Value,
    usage: Option<&Value>,
) -> Option<String> {
    let plan = status.get("codingplan_free").unwrap_or(status);
    let plan_type = plan.get("plan_type").and_then(Value::as_str).unwrap_or("");
    let remaining_days = plan.get("remaining_days").and_then(Value::as_i64).unwrap_or(0);
    let claimed_at = plan.get("claimed_at").and_then(Value::as_str).unwrap_or("");
    // usage 返回的 end_date 就是服务端当天的日期，用它判断「今天是否已签到」，避免重复领取。
    let server_today = usage
        .and_then(|body| body.get("end_date"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if plan_claimed_today(remaining_days, claimed_at, server_today) {
        return Some("今日已签到".to_string());
    }
    if !claim_due(
        remaining_days,
        plan_type,
        LAST_CLAIM.lock().ok().map(|guard| *guard)?,
        now(),
    ) {
        return None;
    }

    let outcome = async {
        let response = client
            .post(format!("{ATOMCODE_BASE}{ATOMCODE_CLAIM_PATH}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {}", auth.access_token))
            .json(&json!({ "plan_type": plan_type }))
            .send()
            .await
            .ok()?;
        let status_code = response.status();
        let text = response.text().await.ok()?;
        Some((status_code, text))
    }
    .await;

    let note = match outcome {
        Some((code, text)) if code.is_success() => {
            let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let message = body
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("签到成功");
            if body.get("duplicate").and_then(Value::as_bool).unwrap_or(false) {
                format!("今日已签到（{message}）")
            } else {
                format!("签到成功（{message}）")
            }
        }
        Some((code, text)) => format!(
            "签到失败：HTTP {code} {}",
            safe_error_body(&text, &[auth.access_token.as_str()])
        ),
        None => "签到失败：请求未完成".to_string(),
    };

    if let Ok(mut guard) = LAST_CLAIM.lock() {
        guard.1 = Some(now());
        if !note.starts_with("签到失败") {
            guard.0 = Some(now());
        }
    }
    Some(note)
}

/// 服务端认为今天已经领过（claimed_at 与明细接口的 end_date 同为今天），且权益仍在有效期内。
fn plan_claimed_today(remaining_days: i64, claimed_at: &str, server_today: &str) -> bool {
    remaining_days > 0 && !claimed_at.is_empty() && claimed_at == server_today
}

/// 是否需要签到：没有可领类型、刚签到过、或刚失败过都不必再打接口。
fn claim_due(
    remaining_days: i64,
    plan_type: &str,
    (last_success, last_attempt): (Option<i64>, Option<i64>),
    now_epoch: i64,
) -> bool {
    if plan_type.is_empty() {
        return false;
    }
    if last_attempt
        .map(|at| now_epoch - at < CLAIM_RETRY_SECS)
        .unwrap_or(false)
    {
        return false;
    }
    // 权益已失效时立刻再签一次（不受 20 小时静默期限制），否则一天一次。
    if remaining_days <= 0 {
        return true;
    }
    last_success
        .map(|at| now_epoch - at >= CLAIM_INTERVAL_SECS)
        .unwrap_or(true)
}

struct Auth {
    access_token: String,
    username: Option<String>,
}

async fn get_json(client: &Client, path: &str, auth: &Auth) -> Result<Value, String> {
    let response = client
        .get(format!("{ATOMCODE_BASE}{path}"))
        .header(header::ACCEPT, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", auth.access_token))
        .send()
        .await
        .map_err(|error| format!("AtomCode 请求失败：{error}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("AtomCode 返回内容无法读取：{error}"))?;
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err("AtomCode 登录已过期，请运行 atomcode 重新登录".into());
    }
    if !status.is_success() {
        return Err(format!(
            "AtomCode 服务返回 HTTP {status}：{}",
            safe_error_body(&text, &[auth.access_token.as_str()])
        ));
    }
    serde_json::from_str(&text).map_err(|error| format!("AtomCode 返回内容无法解析：{error}"))
}

fn auth_path() -> PathBuf {
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        return PathBuf::from(home).join(".atomcode").join("auth.toml");
    }
    PathBuf::from(".atomcode").join("auth.toml")
}

/// 只解析 `key = "value"` 形态的顶层键值，避免为一份本地凭据文件引入 TOML 依赖。
fn read_auth() -> Result<Auth, String> {
    let path = auth_path();
    let text = std::fs::read_to_string(&path).map_err(|_| {
        format!("未找到 {}，请先安装并登录 AtomCode", path.to_string_lossy())
    })?;
    let mut access_token = None;
    let mut username = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim();
        match key.trim() {
            "access_token" => access_token = Some(value.to_owned()),
            "username" | "name" if username.is_none() && !value.is_empty() => {
                username = Some(value.to_owned())
            }
            _ => {}
        }
    }
    let access_token = access_token
        .filter(|token| !token.is_empty())
        .ok_or_else(|| "AtomCode 登录态中没有 access_token，请运行 atomcode 重新登录".to_string())?;
    Ok(Auth {
        access_token,
        username,
    })
}

fn parse_status(
    body: &Value,
    usage: Option<&Value>,
    username: Option<&str>,
) -> Result<ProviderUsage, String> {
    let plan = body.get("codingplan_free").unwrap_or(body);
    let plan_name = plan.get("plan_name").and_then(Value::as_str).map(str::to_owned);
    let total_days = plan.get("total_days").and_then(Value::as_i64).unwrap_or(0);
    let remaining_days = plan.get("remaining_days").and_then(Value::as_i64).unwrap_or(0);
    let expires_at = plan.get("expires_at").and_then(Value::as_str);
    let window = body.get("current_usage").unwrap_or(&Value::Null);

    let mut windows = Vec::new();
    let window_limit = window.get("window_token_limit").and_then(as_number);
    let window_used = window.get("window_tokens_used").and_then(as_number).unwrap_or(0.0);
    let window_hours = window.get("window_hours").and_then(as_number);
    let has_window = !matches!(window.get("placeholder"), Some(Value::Bool(true)))
        && window_limit.map(|limit| limit > 0.0).unwrap_or(false);

    if has_window {
        let limit = window_limit.unwrap_or(0.0);
        let percent = window
            .get("usage_percent")
            .and_then(as_number)
            .unwrap_or_else(|| if limit > 0.0 { window_used / limit * 100.0 } else { 0.0 });
        windows.push(UsageWindow {
            label: match window_hours {
                Some(hours) => format!("{hours} 小时窗口"),
                None => "当前窗口".into(),
            },
            used_percent: percent.clamp(0.0, 100.0).round() as u8,
            resets_at: window
                .get("seconds_until_reset")
                .and_then(Value::as_i64)
                .filter(|seconds| *seconds > 0)
                .map(|seconds| now() + seconds),
        });
    }

    if total_days > 0 {
        let used_days = (total_days - remaining_days).clamp(0, total_days);
        windows.push(UsageWindow {
            label: match expires_at {
                Some(date) => format!("套餐有效期至 {date}（剩余 {remaining_days} 天）"),
                None => "套餐有效期".into(),
            },
            used_percent: (used_days as f64 / total_days as f64 * 100.0).round() as u8,
            resets_at: None,
        });
    }

    let metrics = usage.map(usage_metrics).unwrap_or_default();
    if windows.is_empty() && metrics.is_empty() {
        return Err("AtomCode 未返回可用的用量或套餐信息".into());
    }

    let account = match (username, plan_name.as_deref()) {
        (Some(name), Some(plan)) => Some(format!("{name} · {plan}")),
        (Some(name), None) => Some(name.to_owned()),
        (None, Some(plan)) => Some(plan.to_owned()),
        (None, None) => None,
    };

    let credits = if has_window {
        let limit = window_limit.unwrap_or(0.0);
        Some(Credits {
            remaining: (limit - window_used).max(0.0),
            total: limit,
            plan: plan_name.clone(),
        })
    } else {
        None
    };

    Ok(ProviderUsage {
        provider: "atomcode".into(),
        status: "connected".into(),
        account,
        windows,
        credits,
        metrics,
        updated_at: Some(now()),
        error: None,
    })
}

/// `/coding-plan/usage` 的汇总与最近一天明细。
fn usage_metrics(usage: &Value) -> Vec<UsageMetric> {
    let days = usage.get("days").and_then(Value::as_i64).unwrap_or(60);
    let latest = usage
        .get("rows")
        .and_then(Value::as_array)
        .and_then(|rows| rows.last());
    let latest_date = latest
        .and_then(|row| row.get("date"))
        .and_then(Value::as_str)
        .map(|date| format!("{date} Token"))
        .unwrap_or_default();
    [
        metric(format!("近 {days} 天 Token"), usage.get("total_tokens")),
        metric(format!("近 {days} 天调用"), usage.get("total_counts")),
        metric(latest_date, latest.and_then(|row| row.get("total_tokens"))),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn metric(label: String, value: Option<&Value>) -> Option<UsageMetric> {
    Some(UsageMetric {
        label,
        value: value.and_then(Value::as_f64)?,
    })
}

fn as_number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

#[cfg(test)]
mod tests {
    use super::{claim_due, parse_status, plan_claimed_today};
    use serde_json::json;

    #[test]
    fn parses_window_plan_and_usage_metrics() {
        let usage = parse_status(
            &json!({
                "codingplan_free": {
                    "plan_name": "CodingPlan Pro",
                    "total_days": 30,
                    "remaining_days": 12,
                    "expires_at": "2026-11-01"
                },
                "current_usage": {
                    "placeholder": false,
                    "window_token_limit": 2000000,
                    "window_tokens_used": 500000,
                    "usage_percent": 25,
                    "window_hours": 5,
                    "seconds_until_reset": 3600
                }
            }),
            Some(&json!({
                "days": 60,
                "total_tokens": 5586259,
                "total_counts": 105,
                "rows": [
                    {"date": "2026-10-14", "total_tokens": 0},
                    {"date": "2026-10-15", "total_tokens": 5586259}
                ]
            })),
            Some("SanXiaoXing"),
        )
        .unwrap();
        assert_eq!(usage.account.as_deref(), Some("SanXiaoXing · CodingPlan Pro"));
        assert_eq!(usage.windows.len(), 2);
        assert_eq!(usage.windows[0].label, "5 小时窗口");
        assert_eq!(usage.windows[0].used_percent, 25);
        assert_eq!(usage.windows[1].used_percent, 60);
        assert_eq!(usage.metrics.len(), 3);
        assert_eq!(usage.metrics[0].value, 5_586_259.0);
        assert_eq!(usage.metrics[2].label, "2026-10-15 Token");
        let credits = usage.credits.unwrap();
        assert_eq!(credits.total, 2_000_000.0);
        assert_eq!(credits.remaining, 1_500_000.0);
    }

    #[test]
    fn placeholder_usage_falls_back_to_plan_window() {
        let usage = parse_status(
            &json!({
                "codingplan_free": {
                    "plan_name": "CodingPlan Lite",
                    "total_days": 30,
                    "remaining_days": 0,
                    "expires_at": "2026-09-13"
                },
                "current_usage": { "placeholder": true, "window_token_limit": null }
            }),
            None,
            None,
        )
        .unwrap();
        assert_eq!(usage.windows.len(), 1);
        assert_eq!(usage.windows[0].label, "套餐有效期至 2026-09-13（剩余 0 天）");
        assert_eq!(usage.windows[0].used_percent, 100);
        assert!(usage.credits.is_none());
        assert!(usage.metrics.is_empty());
    }

    #[test]
    fn usage_metrics_keep_working_without_plan() {
        let usage = parse_status(
            &json!({"codingplan_free": {"total_days": 0}}),
            Some(&json!({"days": 60, "total_tokens": 12, "rows": []})),
            None,
        )
        .unwrap();
        assert!(usage.windows.is_empty());
        assert_eq!(usage.metrics[0].label, "近 60 天 Token");
        assert_eq!(usage.metrics[0].value, 12.0);
    }

    #[test]
    fn rejects_empty_payload() {
        assert!(parse_status(&json!({"current_usage": {"placeholder": true}}), None, None).is_err());
    }

    #[test]
    fn claims_once_a_day_per_fresh_process() {
        let now = 1_792_000_000;
        assert!(claim_due(7, "Lite-beta", (None, None), now));
        assert!(claim_due(7, "Lite-beta", (Some(now - 21 * 3600), Some(now - 21 * 3600)), now));
        assert!(!claim_due(7, "Lite-beta", (Some(now - 60), Some(now - 60)), now));
    }

    #[test]
    fn lapsed_plan_claims_but_respects_retry_backoff() {
        let now = 1_792_000_000;
        assert!(claim_due(0, "Lite-beta", (Some(now - 600), Some(now - 3600)), now));
        assert!(!claim_due(0, "Lite-beta", (None, Some(now - 60)), now));
        assert!(!claim_due(0, "", (None, None), now));
    }

    #[test]
    fn detects_claim_already_done_today() {
        assert!(plan_claimed_today(7, "2026-10-16", "2026-10-16"));
        assert!(!plan_claimed_today(7, "2026-10-15", "2026-10-16"));
        assert!(!plan_claimed_today(7, "2026-10-16", ""));
        // 权益已失效时即便 claimed_at 是今天也要重签
        assert!(!plan_claimed_today(0, "2026-10-16", "2026-10-16"));
    }
}
