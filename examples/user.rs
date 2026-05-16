use std::fs::File;
use std::str::FromStr as _;
use dotenvy::dotenv;
use futures::StreamExt as _;
use polymarket_client_sdk_v2::auth::Credentials;
use polymarket_client_sdk_v2::clob::ws::{Client, WsMessage};
use polymarket_client_sdk_v2::types::{Address, B256};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let api_key = Uuid::parse_str(&std::env::var("POLYMARKET_API_KEY")?)?;
    let api_secret = std::env::var("POLYMARKET_API_SECRET")?;
    let api_passphrase = std::env::var("POLYMARKET_API_PASSPHRASE")?;
    let address = Address::from_str(&std::env::var("POLYMARKET_ADDRESS")?)?;

    let credentials = Credentials::new(api_key, api_secret, api_passphrase);

    let client = Client::default().authenticate(credentials, address)?;
    println!("Authenticated successfully");

    let markets: Vec<B256> = Vec::new();
    let mut stream = std::pin::pin!(client.subscribe_user_events(markets)?);
    println!("Subscribed to user events, waiting for messages...");

    let mut count = 0usize;
    while let Some(event) = stream.next().await {
        count += 1;
        println!("[{}] Received event: {:?}", count, event);
        
    }

    println!("Stream ended, total events received: {}", count);
    Ok(())
}