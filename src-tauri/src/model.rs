use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageWindow {
    pub(crate) label: String,
    pub(crate) used_percent: u8,
    pub(crate) resets_at: Option<i64>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Credits {
    pub(crate) remaining: f64,
    pub(crate) total: f64,
    pub(crate) plan: Option<String>,
}

/// 无法用「百分比 + 重置时间」表达的补充指标（例如近 60 天 token 总量）。
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageMetric {
    pub(crate) label: String,
    pub(crate) value: f64,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderUsage {
    pub(crate) provider: String,
    pub(crate) status: String,
    pub(crate) account: Option<String>,
    pub(crate) windows: Vec<UsageWindow>,
    pub(crate) credits: Option<Credits>,
    pub(crate) metrics: Vec<UsageMetric>,
    pub(crate) updated_at: Option<i64>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageSnapshot {
    pub(crate) codex: ProviderUsage,
    pub(crate) workbuddy: ProviderUsage,
    pub(crate) trae: ProviderUsage,
    pub(crate) atomcode: ProviderUsage,
}

pub(crate) struct UsageState(pub(crate) Mutex<UsageSnapshot>);

pub(crate) fn initial() -> UsageSnapshot {
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
        trae: ProviderUsage {
            provider: "trae".into(),
            status: "needs_login".into(),
            ..Default::default()
        },
        atomcode: ProviderUsage {
            provider: "atomcode".into(),
            status: "needs_login".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
