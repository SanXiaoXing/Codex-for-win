use crate::{
    credentials::{read_secret, TRAE_AUTH_KEY, TRAE_COOKIE_KEY, TRAE_UA_KEY},
    model::{Credits, ProviderUsage, UsageWindow},
    providers::{now, number, provider_or_error, safe_error_body},
};
use reqwest::{header, Client};
use serde_json::Value;
use std::sync::Mutex;

const TRAE_ENTITLEMENT_URL: &str =
    "https://api.trae.cn/trae/api/v2/pay/user_current_entitlement_list";
/// 浏览器会话 Cookie 换取短期 JWT 的接口，与 www.trae.cn 前端同款调用。
const TRAE_USER_TOKEN_URL: &str = "https://api.trae.cn/cloudide/api/v3/common/GetUserToken";
/// 提前量：JWT 剩余有效期不足该时长就重新换取。
const TOKEN_REFRESH_MARGIN_SECS: i64 = 600;
const FALLBACK_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// 由浏览器 Cookie 换来的短期 JWT 缓存：(Authorization 值, 过期时间戳)。
static MINTED_TOKEN: Mutex<Option<(String, i64)>> = Mutex::new(None);

pub(crate) async fn fetch_trae(client: &Client) -> ProviderUsage {
    let result = async {
        match read_secret(TRAE_COOKIE_KEY) {
            Ok(cookie) => fetch_with_cookie(client, &cookie).await,
            Err(_) => fetch_with_authorization(client).await,
        }
    }
    .await;

    provider_or_error("trae", result, |error| error.contains("请在设置"))
}

/// 推荐路径：浏览器会话 Cookie → 换取 8 小时 JWT → 查询积分。
/// 每轮刷新都会检查 JWT 剩余有效期，不足则自动重新换取；只要浏览器会话未失效就无需手工重贴。
async fn fetch_with_cookie(client: &Client, cookie: &str) -> Result<ProviderUsage, String> {
    let mut attempt = 0;
    loop {
        let token = session_token(client, cookie).await?;
        match fetch_entitlement(client, &token).await {
            Ok(mut usage) => {
                usage.account = Some(match jwt_expires_at(&token) {
                    Some(expires_at) => format!("TRAE 积分额度 · {}", remaining_label(expires_at)),
                    None => "TRAE 积分额度".into(),
                });
                return Ok(usage);
            }
            Err(error) if attempt == 0 && error.contains("授权已失效") => {
                forget_minted_token();
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

/// 备用路径：使用设置里粘贴的 Authorization 值（8 小时后必然失效，需手工重贴）。
async fn fetch_with_authorization(client: &Client) -> Result<ProviderUsage, String> {
    let stored = read_secret(TRAE_AUTH_KEY)
        .map_err(|_| "请在设置中添加 TRAE 浏览器 Cookie，或粘贴 Authorization 值".to_string())?;
    let mut usage = fetch_entitlement(client, stored.trim()).await?;
    usage.account = Some(match jwt_expires_at(&stored) {
        Some(expires_at) => format!("TRAE 积分额度 · {}", remaining_label(expires_at)),
        None => "TRAE 积分额度".into(),
    });
    Ok(usage)
}

async fn session_token(client: &Client, cookie: &str) -> Result<String, String> {
    if let Ok(guard) = MINTED_TOKEN.lock() {
        if let Some((token, expires_at)) = guard.as_ref() {
            if *expires_at - now() > TOKEN_REFRESH_MARGIN_SECS {
                return Ok(token.clone());
            }
        }
    }

    let user_agent = read_secret(TRAE_UA_KEY).unwrap_or_else(|_| FALLBACK_USER_AGENT.to_string());
    let response = client
        .post(TRAE_USER_TOKEN_URL)
        .header(header::COOKIE, cookie.trim())
        .header(header::USER_AGENT, user_agent.trim())
        .header(header::CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|error| format!("TRAE 换取令牌失败：{error}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("TRAE 令牌响应无法读取：{error}"))?;
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        forget_minted_token();
        return Err("TRAE 浏览器会话已失效，请重新复制 Cookie 与 User-Agent".into());
    }
    if !status.is_success() {
        return Err(format!(
            "TRAE 换取令牌返回 HTTP {status}：{}",
            safe_error_body(&text, &[cookie.trim()])
        ));
    }
    let body: Value = serde_json::from_str(&text)
        .map_err(|error| format!("TRAE 令牌响应无法解析：{error}"))?;
    let token = body
        .pointer("/Result/Token")
        .or_else(|| body.pointer("/data/Result/Token"))
        .or_else(|| body.pointer("/data/Token"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "TRAE 令牌响应结构异常：{}",
                safe_error_body(&text, &[cookie.trim()])
            )
        })?
        .to_owned();
    let authorization = format!("Cloud-IDE-JWT {token}");
    // 解不出 exp 时按接口实测的 8 小时保守处理。
    let expires_at = jwt_expires_at(&authorization).unwrap_or_else(|| now() + 8 * 3600);
    if let Ok(mut guard) = MINTED_TOKEN.lock() {
        *guard = Some((authorization.clone(), expires_at));
    }
    Ok(authorization)
}

fn forget_minted_token() {
    if let Ok(mut guard) = MINTED_TOKEN.lock() {
        *guard = None;
    }
}

async fn fetch_entitlement(client: &Client, authorization: &str) -> Result<ProviderUsage, String> {
    let response = client
        .post(TRAE_ENTITLEMENT_URL)
        .header(header::AUTHORIZATION, authorization.trim())
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|error| format!("TRAE 请求失败：{error}"))?;
    let status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|error| format!("TRAE 返回内容无法读取：{error}"))?;
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err("TRAE 授权已失效，请重新复制 Authorization 值".to_string());
    }
    if !status.is_success() {
        return Err(format!(
            "TRAE 服务返回 HTTP {status}：{}",
            safe_error_body(&response_text, &[authorization.trim()])
        ));
    }
    let body: Value = serde_json::from_str(&response_text)
        .map_err(|error| format!("TRAE 返回内容无法解析：{error}"))?;
    parse_entitlement(&body)
}

fn parse_entitlement(body: &Value) -> Result<ProviderUsage, String> {
    let payload = body.get("data").unwrap_or(body);
    let code = body
        .get("code")
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
        .unwrap_or(0);
    if code != 0 {
        return Err(body
            .get("message")
            .or_else(|| body.get("msg"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("TRAE 权益接口返回错误码 {code}")));
    }

    let summary = payload.get("usage_summary").ok_or_else(|| {
        format!(
            "TRAE 权益响应中没有 usage_summary；字段：[{}]",
            object_keys(payload)
        )
    })?;
    let consumed = number(summary.get("consumed_amount"));
    let total = number(summary.get("total_amount"));
    if total <= 0.0 {
        return Err("TRAE 权益响应中的 total_amount 无效或为 0".into());
    }
    let percent = summary
        .get("consumption_ratio")
        .and_then(as_number)
        .map(|ratio| ratio * 100.0)
        .unwrap_or_else(|| consumed / total * 100.0)
        .clamp(0.0, 100.0);
    let remaining = (total - consumed).max(0.0);
    let updated_at = now();
    Ok(ProviderUsage {
        provider: "trae".into(),
        status: "connected".into(),
        account: Some("TRAE 积分额度".into()),
        windows: vec![UsageWindow {
            label: "当前周期".into(),
            used_percent: percent.round() as u8,
            resets_at: None,
        }],
        credits: Some(Credits {
            remaining,
            total,
            plan: Some("当前周期".into()),
        }),
        updated_at: Some(updated_at),
        error: None,
    })
}

/// 取 JWT 的 exp（不做签名校验，仅用于展示与续期判断）。
fn jwt_expires_at(authorization: &str) -> Option<i64> {
    let raw = authorization.trim().split_whitespace().last()?;
    let segment = raw.split('.').nth(1)?;
    let payload: Value = serde_json::from_slice(&base64_url_decode(segment)?).ok()?;
    payload.get("exp")?.as_i64()
}

fn base64_url_decode(input: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut lookup = [255u8; 256];
    for (index, byte) in TABLE.iter().enumerate() {
        lookup[*byte as usize] = index as u8;
    }
    let mut output = Vec::with_capacity(input.len() / 4 * 3);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        let value = lookup[byte as usize];
        if value == 255 {
            return None;
        }
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
        }
    }
    Some(output)
}

fn remaining_label(expires_at: i64) -> String {
    let seconds = expires_at - now();
    if seconds <= 0 {
        "授权已过期".into()
    } else if seconds >= 3600 {
        format!("授权剩余 {:.1} 小时", seconds as f64 / 3600.0)
    } else {
        format!("授权剩余 {} 分钟", (seconds / 60).max(1))
    }
}

fn object_keys(value: &Value) -> String {
    value
        .as_object()
        .map(|object| {
            object
                .keys()
                .take(12)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "非对象".into())
}

fn as_number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|string| string.parse().ok()))
}

#[cfg(test)]
mod tests {
    use super::{base64_url_decode, jwt_expires_at, parse_entitlement, safe_error_body};
    use serde_json::json;

    #[test]
    fn parses_dashboard_credit_summary() {
        let usage = parse_entitlement(&json!({
            "is_credits_billing": true,
            "usage_summary": {
                "consumed_amount": 871.64,
                "consumption_ratio": 0.4587578947368421,
                "total_amount": 1900
            }
        }))
        .unwrap();
        let credits = usage.credits.unwrap();
        assert!((credits.remaining - 1028.36).abs() < 0.0001);
        assert_eq!(credits.total, 1900.0);
        assert_eq!(usage.windows[0].used_percent, 46);
    }

    #[test]
    fn parses_wrapped_dashboard_payload_and_numeric_strings() {
        let usage = parse_entitlement(&json!({"code": 0, "data": {"usage_summary": {
            "consumed_amount": "12.5", "total_amount": "100"
        }}}))
        .unwrap();
        assert_eq!(usage.credits.unwrap().remaining, 87.5);
        assert_eq!(usage.windows[0].used_percent, 13);
    }

    #[test]
    fn reports_nonzero_api_code() {
        let error = match parse_entitlement(&json!({"code": 1001, "message": "expired"})) {
            Ok(_) => panic!("nonzero API code should fail"),
            Err(error) => error,
        };
        assert_eq!(error, "expired");
    }

    #[test]
    fn error_details_hide_authorization() {
        assert_eq!(
            safe_error_body(
                "bad request secret-token device-123",
                &["secret-token", "device-123"]
            ),
            "bad request [已隐藏] [已隐藏]"
        );
    }

    #[test]
    fn decodes_jwt_expiry_without_padding() {
        // {"exp":1700000000} 的 base64url 片段
        let payload = "eyJleHAiOjE3MDAwMDAwMDB9";
        assert_eq!(
            jwt_expires_at(&format!("Cloud-IDE-JWT x.{payload}.sig")),
            Some(1_700_000_000)
        );
        assert_eq!(jwt_expires_at("not-a-jwt"), None);
    }

    #[test]
    fn base64_url_decode_handles_padding_and_rejects_garbage() {
        assert_eq!(
            base64_url_decode("eyJhIjoxfQ").as_deref(),
            Some(&b"{\"a\":1}"[..])
        );
        assert_eq!(
            base64_url_decode("eyJhIjoxfQ==").as_deref(),
            Some(&b"{\"a\":1}"[..])
        );
        assert_eq!(base64_url_decode("bad*value"), None);
    }
}
