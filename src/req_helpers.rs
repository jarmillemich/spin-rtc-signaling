#![allow(unused)]
use std::collections::HashMap;
use anyhow::{anyhow,Result};
use serde_json::Value;
use spin_sdk::http::Request;
use urlencoding::decode;

/// Takes some key/value pair from a query string and url-decods both sides
fn decode_query_pair(key_value: (&str, &str)) -> Result<(String, String)> {
    let (key, value) = key_value;
    let key = decode(key)?.into_owned();
    let value = decode(value)?.into_owned();
    Ok((key, value))
}

/// Returns a HashMap over the query/search parameters on this requrest
pub fn parse_query(req: &Request) -> Result<HashMap<String, String>> {
    let query = req.uri().query()
        .ok_or_else(|| anyhow!("No query"))?;
    
    querystring::querify(query)
        .into_iter()
        .map(decode_query_pair)
        .collect::<Result<HashMap<String, String>>>()

}

/// Returns the value of the specified key in the query, or an Err if not present
pub fn required_query<'a>(query: &'a HashMap<String, String>, key: &str) -> Result<&'a String> {
    let value = query
        .get(key)
        .ok_or_else(|| anyhow!(format!("missing required parameter {key}")))?;
    
    Ok(value)
}

/// Returns the specified string value from a json object, or an Err
pub fn required_json_str<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| anyhow!(format!("missing required parameter {key}")))
}

/// Returns the specified string value from a json object, or an Err
pub fn required_json_i64(value: &Value, key: &str) -> Result<i64> {
    value[key]
        .as_i64()
        .ok_or_else(|| anyhow!(format!("missing required parameter {key}")))
}

/// Returns the specified string value from a json object, or an Err
pub fn required_json_bool(value: &Value, key: &str) -> Result<bool> {
    value[key]
        .as_bool()
        .ok_or_else(|| anyhow!(format!("missing required parameter {key}")))
}
