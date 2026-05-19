use anyhow::{Result, anyhow};
use serde_json::Value;

pub(crate) fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing required parameter: {key}"))
}

pub(crate) fn required_u32(args: &Value, key: &str) -> Result<u32> {
    let v = args
        .get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow!("Missing required parameter: {key}"))?;
    u32::try_from(v).map_err(|_| anyhow!("Parameter '{key}' value {v} exceeds u32::MAX"))
}
