use std::fs::File;
use std::str::FromStr as _;

use futures::StreamExt as _;
use polymarket_client_sdk_v2::clob::ws::Client;
use polymarket_client_sdk_v2::types::U256;
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use dotenvy::dotenv;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    if let Ok(path) = std::env::var("LOG_FILE") {
        let file = File::create(path)?;
        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(file)
                    .with_ansi(false),
            )
            .init();
    } else {
        tracing_subscriber::fmt::init();
    }

    let client = Client::default();
    info!(endpoint = "websocket", "connected to CLOB WebSocket API");

    let asset_ids = vec![
        U256::from_str(
            "64559207542767597021637818520033870602190377483912209920163945084657085847117",
        )?
    ];

    let stream = client.subscribe_orderbook(asset_ids.clone())?;
    let mut stream = Box::pin(stream);
    info!(
        endpoint = "subscribe_orderbook",
        asset_count = asset_ids.len(),
        "subscribed to orderbook updates"
    );

    while let Some(book_result) = stream.next().await {
        println!("!!!!!!!!!!!!!!!!!!");
        match book_result {
            Ok(book) => {
                info!(
                    endpoint = "orderbook",
                    asset_id = %book.asset_id,
                    market = %book.market,
                    timestamp = %book.timestamp,
                    bids = book.bids.len(),
                    asks = book.asks.len()
                );

                for (i, bid) in book.bids.iter().take(5).enumerate() {
                    debug!(
                        endpoint = "orderbook",
                        side = "bid",
                        rank = i + 1,
                        size = %bid.size,
                        price = %bid.price
                    );
                }

                for (i, ask) in book.asks.iter().take(5).enumerate() {
                    debug!(
                        endpoint = "orderbook",
                        side = "ask",
                        rank = i + 1,
                        size = %ask.size,
                        price = %ask.price
                    );
                }

                if let Some(hash) = &book.hash {
                    debug!(endpoint = "orderbook", hash = %hash);
                }
            }
            Err(e) => error!(endpoint = "orderbook", error = %e),
        }
    }

    Ok(())
}