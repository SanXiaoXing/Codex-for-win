use crate::{
    model::{ProviderUsage, UsageWindow},
    providers::{now, provider_or_error},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::{header, Client};
use serde_json::Value;
use std::{fs, path::PathBuf};

const CODEX_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

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

pub(crate) async fn fetch_codex(client: &Client) -> ProviderUsage {
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
