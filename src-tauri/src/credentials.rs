use keyring::Entry;

const SERVICE: &str = "codexbar-windows-mvp";
pub(crate) const WORKBUDDY_COOKIE_KEY: &str = "workbuddy-cookie";
pub(crate) const WORKBUDDY_UA_KEY: &str = "workbuddy-user-agent";
pub(crate) const TRAE_AUTH_KEY: &str = "trae-authorization";

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

pub(crate) fn write_secret(key: &str, value: &str) -> Result<(), String> {
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

pub(crate) fn read_secret(key: &str) -> Result<String, String> {
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
