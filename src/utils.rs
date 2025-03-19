use anyhow::Result;
use serde::Serialize;

pub fn to_json<T>(input: T) -> Result<String>
where
    T: Sized + Serialize,
{
    let res = serde_json::to_string(&input).unwrap();
    Ok(res)
}
