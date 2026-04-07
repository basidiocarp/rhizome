use anyhow::{Result, anyhow};
use serde_json::Value;

pub(crate) fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing required parameter: {key}"))
}

pub(crate) fn required_u32(args: &Value, key: &str) -> Result<u32> {
    args.get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .ok_or_else(|| anyhow!("Missing required parameter: {key}"))
}
