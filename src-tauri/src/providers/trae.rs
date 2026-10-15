use crate::{
    credentials::{read_secret, TRAE_AUTH_KEY},
    model::{Credits, ProviderUsage, UsageWindow},
    providers::{now, number, provider_or_error, safe_error_body},
};
use reqwest::Client;
use serde_json::Value;

const TRAE_ENTITLEMENT_URL: &str =
    "https://api.trae.cn/trae/api/v2/pay/user_current_entitlement_list";
pub(crate) async fn fetch_trae(client: &Client) -> ProviderUsage {
    let result = async {
        let authorization =
            read_secret(TRAE_AUTH_KEY).map_err(|_| "请在设置中添加 TRAE 授权值".to_string())?;
        let request = client
            .post(TRAE_ENTITLEMENT_URL)
            .header(reqwest::header::AUTHORIZATION, authorization.trim())
            .json(&serde_json::json!({}));
        let response = request
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
    .await;

    provider_or_error("trae", result, |error| error.contains("添加 TRAE"))
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
    use super::{parse_entitlement, safe_error_body};
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
}
