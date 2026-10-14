use reqwest::Client;
use serde_json::Value;

const GET_USER_TOKEN_URL: &str = "https://api.trae.cn/cloudide/api/v3/common/GetUserToken";
const CLAIM_CREDITS_URL: &str = "https://api.trae.cn/trae/api/v2/ug/checkin_credits/claim";

pub(crate) async fn check_in(session: &str, device_id: &str) -> Result<String, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .map_err(|error| format!("无法创建 TRAE 请求：{error}"))?;

    let token_response = client
        .post(GET_USER_TOKEN_URL)
        .header(
            reqwest::header::COOKIE,
            format!("X-Cloudide-Session={}", session.trim()),
        )
        .header(reqwest::header::REFERER, "https://www.trae.cn/")
        .header(reqwest::header::ORIGIN, "https://www.trae.cn")
        .header(reqwest::header::USER_AGENT, "TraeCheckin/1.0")
        .header(reqwest::header::ACCEPT, "application/json, text/plain, */*")
        .body("")
        .send()
        .await
        .map_err(|error| format!("获取 TRAE 签到令牌失败：{error}"))?;
    let status = token_response.status();
    let response_text = token_response
        .text()
        .await
        .map_err(|error| format!("读取 TRAE 签到令牌响应失败：{error}"))?;
    if !status.is_success() {
        return Err(format!(
            "TRAE GetUserToken 返回 HTTP {status}：{}",
            safe_error_body(&response_text, &[session, device_id])
        ));
    }
    let token_body: Value = serde_json::from_str(&response_text)
        .map_err(|error| format!("TRAE GetUserToken 响应无法解析：{error}"))?;
    let token = token_body
        .pointer("/Result/Token")
        .or_else(|| token_body.pointer("/result/token"))
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            let message = token_body
                .get("message")
                .or_else(|| token_body.get("Message"))
                .and_then(Value::as_str)
                .unwrap_or("响应中没有 Result.Token");
            format!("TRAE GetUserToken 失败：{message}")
        })?;

    let checkin_response = client
        .post(CLAIM_CREDITS_URL)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Cloud-IDE-JWT {token}"),
        )
        .header("X-User-Region", "cn")
        .header("x-device-id", device_id.trim())
        .header(reqwest::header::REFERER, "https://www.trae.cn/")
        .header(reqwest::header::ORIGIN, "https://www.trae.cn")
        .header(reqwest::header::USER_AGENT, "TraeCheckin/1.0")
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|error| format!("发送 TRAE 签到请求失败：{error}"))?;
    let status = checkin_response.status();
    let response_text = checkin_response
        .text()
        .await
        .map_err(|error| format!("读取 TRAE 签到响应失败：{error}"))?;
    if !status.is_success() {
        return Err(format!(
            "TRAE 签到接口返回 HTTP {status}：{}",
            safe_error_body(&response_text, &[session, token, device_id])
        ));
    }
    let body: Value = serde_json::from_str(&response_text)
        .map_err(|error| format!("TRAE 签到响应无法解析：{error}"))?;
    checkin_result_message(&body)
}

fn checkin_result_message(body: &Value) -> Result<String, String> {
    let code = body
        .get("code")
        .or_else(|| body.get("Code"))
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
        .unwrap_or(-1);
    let message = body
        .get("message")
        .or_else(|| body.get("Message"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if code != 0 {
        return Err(if message.is_empty() {
            format!("TRAE 签到失败，接口错误码 {code}")
        } else {
            format!("TRAE 签到失败：{message}（错误码 {code}）")
        });
    }

    if body
        .get("checked_in")
        .or_else(|| body.get("CheckedIn"))
        .and_then(Value::as_bool)
        == Some(true)
    {
        return Ok("今日已完成签到".into());
    }
    let credits = body
        .get("credits")
        .or_else(|| body.get("Credits"))
        .and_then(as_number)
        .unwrap_or(0.0);
    if credits > 0.0 {
        Ok(format!("签到成功，获得 {:.0} 积分", credits))
    } else {
        Ok("TRAE 已接受签到请求，返回 0 积分；可能今日已签到".into())
    }
}

fn safe_error_body(body: &str, secrets: &[&str]) -> String {
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

fn as_number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|string| string.parse().ok()))
}

#[cfg(test)]
mod tests {
    use super::{checkin_result_message, safe_error_body};
    use serde_json::json;

    #[test]
    fn recognizes_checkin_success_and_duplicate_checkin() {
        assert_eq!(
            checkin_result_message(&json!({"code": 0, "credits": 150})).unwrap(),
            "签到成功，获得 150 积分"
        );
        assert_eq!(
            checkin_result_message(&json!({"code": 0, "checked_in": true})).unwrap(),
            "今日已完成签到"
        );
    }

    #[test]
    fn reports_checkin_business_errors() {
        let result = checkin_result_message(&json!({"code": 9074, "message": "稍后再试"}));
        assert!(result.unwrap_err().contains("稍后再试"));
    }

    #[test]
    fn redacts_secrets_from_checkin_errors() {
        assert_eq!(
            safe_error_body(
                "request failed secret-token device-123",
                &["secret-token", "device-123"]
            ),
            "request failed [已隐藏] [已隐藏]"
        );
    }
}
