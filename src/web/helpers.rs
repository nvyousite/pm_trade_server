use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use polymarket_client_sdk_v2::types::{Decimal, U256};
use polymarket_client_sdk_v2::clob::types::{Side};
use std::str::FromStr;

#[derive(Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub data: Option<T>,
    pub error: Option<String>,
}

pub fn bad_request(msg: impl Into<String>) -> (StatusCode, Json<ApiResponse<Value>>) {
    let resp = ApiResponse { code: 1, data: None, error: Some(msg.into()) };
    (StatusCode::BAD_REQUEST, Json(resp))
}

pub fn internal_error(msg: impl Into<String>) -> (StatusCode, Json<ApiResponse<Value>>) {
    let resp = ApiResponse { code: 1, data: None, error: Some(msg.into()) };
    (StatusCode::INTERNAL_SERVER_ERROR, Json(resp))
}

pub fn ok_with_json(val: Value) -> (StatusCode, Json<ApiResponse<Value>>) {
    let resp = ApiResponse { code: 0, data: Some(val), error: None };
    (StatusCode::OK, Json(resp))
}

pub fn parse_u256(s: &str) -> Result<U256, (StatusCode, Json<ApiResponse<Value>>)> {
    U256::from_str(s).map_err(|e| bad_request(format!("invalid token_id: {}", e)))
}

pub fn parse_decimal(name: &str, s: &str) -> Result<Decimal, (StatusCode, Json<ApiResponse<Value>>)> {
    Decimal::from_str(s).map_err(|e| bad_request(format!("invalid {}: {}", name, e)))
}

pub fn parse_side(s: &str) -> Result<Side, (StatusCode, Json<ApiResponse<Value>>)> {
    let s_lower = s.to_lowercase();
    if s_lower == "buy" {
        Ok(Side::Buy)
    } else if s_lower == "sell" {
        Ok(Side::Sell)
    } else {
        Err(bad_request(format!("invalid side: must be 'buy' or 'sell', got '{}'", s)))
    }
}
