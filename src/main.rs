// Cargo.toml
/*
[package]
name = "deriv-websocket"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = { version = "0.7", features = ["ws"] }
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = { version = "0.23", features = ["native-tls"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
futures-util = "0.3"
tracing = "0.1"
tracing-subscriber = "0.3"

# ✨ เพิ่มใหม่: tower-http สำหรับ CORS
# เพื่อให้ pkderiv.shop เรียก WebSocket API จาก domain อื่นได้
tower-http = { version = "0.5", features = ["cors"] }
*/

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message as TungsteniteMessage};
use tracing::{info, error};

// ✨ เพิ่มใหม่: import CORS จาก tower-http
// ใช้สำหรับอนุญาตให้ website จาก domain อื่นเรียก API ได้
use tower_http::cors::{CorsLayer, Any};

type WsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
>;

// WebSocket client สำหรับเชื่อมต่อ Deriv
struct DerivConnection {
    write: Arc<Mutex<futures_util::stream::SplitSink<WsStream, TungsteniteMessage>>>,
    read: Arc<Mutex<futures_util::stream::SplitStream<WsStream>>>,
}

impl DerivConnection {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let deriv_url = "wss://ws.derivws.com/websockets/v3?app_id=1089";
        info!("Connecting to Deriv API: {}", deriv_url);
        
        let (ws_stream, response) = connect_async(deriv_url).await?;
        info!("Connected to Deriv! Response: {:?}", response.status());
        
        let (write, read) = ws_stream.split();
        
        Ok(Self {
            write: Arc::new(Mutex::new(write)),
            read: Arc::new(Mutex::new(read)),
        })
    }

    async fn send(&self, msg: String) -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = self.write.lock().await;
        writer.send(TungsteniteMessage::Text(msg)).await?;
        Ok(())
    }

    async fn receive(&self) -> Option<String> {
        let mut reader = self.read.lock().await;
        while let Some(msg) = reader.next().await {
            match msg {
                Ok(TungsteniteMessage::Text(text)) => return Some(text),
                Ok(TungsteniteMessage::Close(_)) => {
                    info!("Deriv connection closed");
                    return None;
                }
                Err(e) => {
                    error!("Error receiving from Deriv: {}", e);
                    return None;
                }
                _ => continue,
            }
        }
        None
    }
}

// Handler สำหรับ WebSocket endpoint
async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    info!("New WebSocket connection request");
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(socket: WebSocket) {
    info!("WebSocket connection established");
    
    // สร้างการเชื่อมต่อไป Deriv
    let deriv = match DerivConnection::new().await {
        Ok(conn) => {
            info!("Successfully connected to Deriv");
            Arc::new(conn)
        }
        Err(e) => {
            error!("Failed to connect to Deriv: {}", e);
            return;
        }
    };

    let (mut sender, mut receiver) = socket.split();

    // Clone สำหรับใช้ใน task
    let deriv_for_send = deriv.clone();
    let deriv_for_recv = deriv.clone();

    // Task 1: รับข้อมูลจาก client และส่งไป Deriv
    let mut recv_task = tokio::spawn(async move {
        info!("Started client->deriv task");
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                info!("Received from client: {}", text);
                
                if let Err(e) = deriv_for_send.send(text).await {
                    error!("Error sending to Deriv: {}", e);
                    break;
                }
            } else if let Message::Close(_) = msg {
                info!("Client closed connection");
                break;
            }
        }
        info!("Client->Deriv task ended");
    });

    // Task 2: รับข้อมูลจาก Deriv และส่งไป client
    let mut send_task = tokio::spawn(async move {
        info!("Started deriv->client task");
        while let Some(response) = deriv_for_recv.receive().await {
            info!("Received from Deriv: {}", response);
            
            if sender.send(Message::Text(response)).await.is_err() {
                error!("Error sending to client");
                break;
            }
        }
        info!("Deriv->Client task ended");
    });

    // รอให้ task หนึ่งจบ
    tokio::select! {
        _ = &mut recv_task => {
            info!("Recv task completed first");
            send_task.abort();
        }
        _ = &mut send_task => {
            info!("Send task completed first");
            recv_task.abort();
        }
    }
    
    info!("WebSocket handler completed");
}

// Root handler
async fn root_handler() -> &'static str {
    "✅ Deriv WebSocket Proxy Server is running!\n\nConnect to: ws://localhost:3000/ws"
}

// Health check
async fn health_handler() -> impl IntoResponse {
    json!({
        "status": "ok",
        "websocket": "ws://localhost:3000/ws"
    }).to_string()
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // ✨✨✨ เพิ่มใหม่: CORS Layer ✨✨✨
    // อนุญาตให้ทุก origin (domain) เรียก API ได้
    // ถ้าต้องการจำกัดเฉพาะ pkderiv.shop ใช้:
    // .allow_origin("https://pkderiv.shop".parse::<HeaderValue>().unwrap())
    let cors = CorsLayer::new()
        .allow_origin(Any)      // อนุญาตทุก domain (สำหรับ production ควรระบุ domain ที่ชัดเจน)
        .allow_methods(Any)     // อนุญาตทุก HTTP method (GET, POST, etc.)
        .allow_headers(Any);    // อนุญาตทุก headers

    // สร้าง router
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/health", get(health_handler))
        .route("/ws", get(ws_handler))
        .layer(cors);  // ✨ เพิ่ม CORS layer เข้าไป

    // ✨ สำคัญ: ใช้ 0.0.0.0 แทน 127.0.0.1
    // เพื่อให้รับ connection จากภายนอก EC2 ได้
    // ถ้าใช้ 127.0.0.1 จะรับได้แค่ localhost เท่านั้น
    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    
    println!("\n🚀 Server is running!");
    println!("📍 Address: http://{}", addr);
    println!("🔌 WebSocket: ws://localhost:3000/ws");
    println!("🌐 External WebSocket: wss://pkderiv.shop/ws");  // ✨ เพิ่มบรรทัดนี้
    println!("❤️  Health check: http://localhost:3000/health\n");
    println!("🌐❤️🌐  html local page: open  test.html in live server หรือ http://127.0.0.1:5500/test.html  mode \n");

    // Start server
    axum::serve(listener, app).await.unwrap();
}

// ==================== Helper Functions ====================

pub fn ping_request() -> serde_json::Value {
    json!({ "ping": 1 })
}

pub fn ticks_request(symbol: &str) -> serde_json::Value {
    json!({
        "ticks": symbol,
        "subscribe": 1
    })
}

pub fn authorize_request(token: &str) -> serde_json::Value {
    json!({ "authorize": token })
}

pub fn balance_request() -> serde_json::Value {
    json!({
        "balance": 1,
        "subscribe": 1
    })
}

pub fn active_symbols_request() -> serde_json::Value {
    json!({
        "active_symbols": "brief",
        "product_type": "basic"
    })
}

pub fn proposal_request(
    contract_type: &str,
    symbol: &str,
    amount: f64,
    duration: i32,
    duration_unit: &str,
) -> serde_json::Value {
    json!({
        "proposal": 1,
        "amount": amount,
        "basis": "stake",
        "contract_type": contract_type,
        "currency": "USD",
        "duration": duration,
        "duration_unit": duration_unit,
        "symbol": symbol
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_builders() {
        let ping = ping_request();
        assert_eq!(ping["ping"], 1);

        let ticks = ticks_request("R_50");
        assert_eq!(ticks["ticks"], "R_50");
    }
}