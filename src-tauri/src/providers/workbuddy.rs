use crate::{
    credentials::{read_secret, WORKBUDDY_COOKIE_KEY, WORKBUDDY_UA_KEY},
    model::{Credits, ProviderUsage},
    providers::{now, number, provider_or_error},
};
use reqwest::{header, Client};
use serde_json::{json, Value};

const WORKBUDDY_URL: &str = "https://www.workbuddy.cn/billing/meter/get-user-resource-summary";

pub(crate) async fn fetch_workbuddy(client: &Client) -> ProviderUsage {
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
                plan,
            }),
            metrics: Vec::new(),
            updated_at: Some(now()),
            error: None,
        })
    }
    .await;
    provider_or_error("workbuddy", result, |error| error.contains("请先在设置"))
}
