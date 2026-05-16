use alloy::primitives::Address;
use dotenvy::dotenv;
use std::str::FromStr as _;

// Use aliased older alloy for compatibility with SDK's expected Signer trait
use alloy_1_8::signers::local::LocalSigner;
use alloy_1_8::signers::Signer as _;
use polymarket_client_sdk_v2::clob::types::{OrderType, Side, SignatureType};
use polymarket_client_sdk_v2::clob::types::response::PostOrderResponse;
use polymarket_client_sdk_v2::types::Decimal;
use polymarket_client_sdk_v2::clob::{Client, Config};
use polymarket_client_sdk_v2::types::U256;
use polymarket_client_sdk_v2::{POLYGON, PRIVATE_KEY_VAR};
use polymarket_client_sdk_v2::auth::state::Authenticated;
use polymarket_client_sdk_v2::auth::Normal;
use axum::{routing::{get, post}, Json, Router, extract::State, response::IntoResponse};
use serde::Deserialize;
use polymarket_client_sdk_v2::clob::types::request::OrdersRequest;
// Removed unused imports (StatusCode, Value) — moved helpers to `api::response`.
use std::net::SocketAddr;
use std::sync::Arc;
use k256::ecdsa::SigningKey;
mod web;

#[derive(Deserialize)]
struct OrderRequest {
    token_id: String,
    price: String,
    size: String,
    side: String,
}

use crate::web::helpers as web_helpers;

#[derive(Clone)]
struct AppState {
    // shared client so handlers can reuse the authenticated client
    client: Arc<Client<Authenticated<Normal>>>,
    // shared signer so handlers can sign without reconstructing from env
    signer: Arc<LocalSigner<SigningKey>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let signer = LocalSigner::from_str(&std::env::var(PRIVATE_KEY_VAR)?)?.with_chain_id(Some(POLYGON));

    let client = get_client(&signer).await?;

    // start http server with shared client and signer in state
    let signer_obj = Arc::new(signer);
    let state = AppState { client: Arc::new(client), signer: signer_obj };

    // Build our application with a route
    let app = Router::new()
        .route("/order", post(order_handler))
        .route("/cancel", post(cancel_handler))
        .route("/balance", get(balance_handler))
        .route("/orders", get(orders_handler))
        .with_state(state);

    // run it
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let addr = SocketAddr::from(([127, 0, 0, 1], port.parse().unwrap_or(3000)));
    println!("Listening on http://{}", addr);
    tokio::spawn(async move { axum::Server::bind(&addr).serve(app.into_make_service()).await.unwrap() });
    tokio::signal::ctrl_c().await?;
    Ok(())
}

async fn order_handler(
    State(state): State<AppState>,
    Json(payload): Json<OrderRequest>,
) -> impl IntoResponse {
    // parse inputs with helpers
    let token_id = match web_helpers::parse_u256(&payload.token_id) { Ok(v) => v, Err(resp) => return resp };
    let price = match web_helpers::parse_decimal("price", &payload.price) { Ok(v) => v, Err(resp) => return resp };
    let size = match web_helpers::parse_decimal("size", &payload.size) { Ok(v) => v, Err(resp) => return resp };
    let side = match web_helpers::parse_side(&payload.side) { Ok(v) => v, Err(resp) => return resp };

    // reuse the authenticated client and signer created at startup
    let client = state.client.clone();
    let signer = state.signer.as_ref();
    match place_order(&*client, signer, token_id, price, size, side).await {
        Ok(post_resp) => {
            let data_val = serde_json::json!({
                "order_id": post_resp.order_id,
                "status": post_resp.status,
            });
            web_helpers::ok_with_json(data_val)
        }
        Err(e) => web_helpers::internal_error(format!("order error: {}", e)),
    }
}

#[derive(serde::Deserialize)]
struct CancelRequest {
    // accept either single order_id or array; for simplicity use single now
    order_id: String,
}

async fn cancel_handler(
    State(state): State<AppState>,
    Json(payload): Json<CancelRequest>,
) -> impl IntoResponse {
    // call SDK cancel endpoint
    // This assumes the SDK exposes client.cancel_orders(vec![order_id]).build().post().await style
    let client = state.client.clone();

    let resp = client.cancel_orders(&[payload.order_id.as_str()]).await;

    match resp {
        Ok(r) => {
            let data_val = serde_json::json!({
                "canceled": r.canceled,
                "not_canceled": r.not_canceled,
            });
            web_helpers::ok_with_json(data_val)
        }
        Err(e) => web_helpers::internal_error(format!("cancel error: {}", e)),
    }
}

async fn balance_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client = state.client.clone();
    match get_balance(&client).await {
        Ok(balance) => {
            let data_val = serde_json::json!({
                "balance": balance.to_string(),
            });
            web_helpers::ok_with_json(data_val)
        }
        Err(e) => web_helpers::internal_error(format!("balance query error: {}", e)),
    }
}

async fn orders_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client = state.client.clone();
    
    let req = OrdersRequest::default();
    
    match client.orders(&req, None).await {
        Ok(page) => {
            let orders_json: Vec<serde_json::Value> = page.data.into_iter()
                .map(|order| {
                    serde_json::json!({
                        "id": order.id,
                        "status": order.status,
                        "market": order.market,
                        "side": order.side,
                        "price": order.price.to_string(),
                        "order_type": order.order_type,
                        "asset_id": order.asset_id,
                        "original_size": order.original_size.to_string(),
                        "size_matched": order.size_matched.to_string(),
                        "outcome": order.outcome,
                    })
                })
                .collect();
            web_helpers::ok_with_json(serde_json::json!(orders_json))
        }
        Err(e) => web_helpers::internal_error(format!("orders query error: {}", e)),
    }
}   


// 获取client，使用已有 signer
// 获取client
async fn get_client<S>(signer: &S) -> anyhow::Result<Client<Authenticated<Normal>>>
where
    S: polymarket_client_sdk_v2::auth::Signer + Send + Sync + 'static,
{
    let host =
        std::env::var("CLOB_API_URL").unwrap_or_else(|_| "https://clob.polymarket.com".into());

    let client = Client::new(&host, Config::default())?
        .authentication_builder(signer)
        .funder(Address::from_str(&std::env::var("FUNDER_ADDRESS")?)?)
        .signature_type(SignatureType::GnosisSafe)
        .authenticate()
        .await?;

    Ok(client)
}


// place_order: 使用已认证 client 和兼容的 signer 下单，并在错误时打印
async fn place_order<S>(
    client: &Client<Authenticated<Normal>>,
    signer: &S,
    token_id: U256,
    price: Decimal,
    size: Decimal,
    side: Side
)-> anyhow::Result<PostOrderResponse>
where
    S: polymarket_client_sdk_v2::auth::Signer + Send + Sync + 'static,
{

    if side == Side::Buy {
        let esmilited_cost = price * size * Decimal::from(1000000); 
        let balance: Decimal = get_balance(client).await?;
        if esmilited_cost > balance {
            anyhow::bail!("Insufficient balance: estimated cost={}, balance={}", esmilited_cost, balance);
        }
        println!("Placing BUY order: token_id={}, price={}, size={}, balance={}", token_id, price, size, balance);    
    } else {
        let shares: Decimal = get_token_shares(client, token_id).await?;
        let to_sell = size * Decimal::from(1000000);
        if to_sell > shares {
            anyhow::bail!("Insufficient shares: trying to sell size={}, but only have shares={}", to_sell, shares);
        }
        println!("Placing SELL order: token_id={}, price={}, size={}, shares={}", token_id, price, to_sell, shares);
    }

    
    let resp = client
        .limit_order()
        .token_id(token_id)
        .side(side)
        .price(price)
        .size(size)
        .order_type(OrderType::GTC)
        .build_sign_and_post(signer)
        .await;

    match resp {
        Ok(r) => {
            println!("order_id={} status={}", r.order_id, r.status);
            Ok(r)
        }
        Err(err) => {
            eprintln!("下单失败: {:#}", err);
            Err(err.into())
        }
    }
}

async fn get_balance(
    client: &Client<Authenticated<Normal>>
) -> anyhow::Result<Decimal>
{
    let resp = client
        .balance_allowance(
            polymarket_client_sdk_v2::clob::types::request::BalanceAllowanceRequest::builder()
                .asset_type(polymarket_client_sdk_v2::clob::types::AssetType::Collateral)
                .build(),
        )
        .await?;

    println!("balance: {}", resp.balance);
    Ok(resp.balance)
}

async fn get_token_shares(
    client: &Client<Authenticated<Normal>>,
    token_id: U256,
) -> anyhow::Result<Decimal>
{
    let resp = client
        .balance_allowance(
            polymarket_client_sdk_v2::clob::types::request::BalanceAllowanceRequest::builder()
                .asset_type(polymarket_client_sdk_v2::clob::types::AssetType::Conditional)
                .token_id(token_id)
                .build(),
        )
        .await?;

    println!("balance: {}", resp.balance);
    Ok(resp.balance)
}