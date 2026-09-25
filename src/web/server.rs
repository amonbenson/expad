use defmt::{unwrap, warn};
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_net::Stack;
use picoserve::futures::Either as MessageOrUpdate;
use picoserve::io::{Read, Write};
use picoserve::response::File;
use picoserve::response::ws::{Message, SocketRx, SocketTx, WebSocketCallback, WebSocketUpgrade};
use picoserve::routing::{get, get_service};
use picoserve::{Config, Router, Server};
use serde::Serialize;

use super::interface::{INTERFACE, Settings, Status};

/// HTTP connections served in parallel. Every open browser session keeps one busy.
pub(super) const MAX_SESSIONS: usize = 4;

const PORT: u16 = 80;

/// WebSocket close code telling the browser to reconnect later.
const TRY_AGAIN_LATER: u16 = 1013;

static CONFIG: Config = Config::const_default().keep_connection_alive();

/// Single-file web interface built from web/ by build.rs.
const INDEX_HTML: File = File::with_content_type_and_headers(
    File::MIME_HTML,
    include_bytes!(concat!(env!("OUT_DIR"), "/web/index.html.gz")),
    &[("Content-Encoding", "gzip")],
);

/// Message pushed to the browser, serialized as `{"status": ...}` or `{"settings": ...}`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum Update {
    Status(Status),
    Settings(Settings),
}

/// Streams status and settings to one browser and applies the settings it sends back.
struct InterfaceSession;

impl WebSocketCallback for InterfaceSession {
    async fn run<R: Read, W: Write<Error = R::Error>>(
        self,
        mut rx: SocketRx<R>,
        mut tx: SocketTx<W>,
    ) -> Result<(), W::Error> {
        let (Some(mut status), Some(mut settings)) =
            (INTERFACE.status.receiver(), INTERFACE.settings.receiver())
        else {
            return tx.close((TRY_AGAIN_LATER, "Too many sessions")).await;
        };

        // Receivers only report values sent after their creation, so push the current settings first.
        send(&mut tx, Update::Settings(settings.get().await)).await?;

        // Fits a full settings message (~760 bytes) with every float at full precision.
        let mut buffer = [0; 1024];
        let close_reason = loop {
            let update = select(status.changed(), settings.changed());
            let message = match rx.next_message(&mut buffer, update).await? {
                MessageOrUpdate::First(message) => message,
                MessageOrUpdate::Second(Either::First(value)) => {
                    send(&mut tx, Update::Status(value)).await?;
                    continue;
                }
                MessageOrUpdate::Second(Either::Second(value)) => {
                    send(&mut tx, Update::Settings(value)).await?;
                    continue;
                }
            };

            match message {
                Ok(Message::Text(text)) => match serde_json_core::from_str(text) {
                    Ok((new_settings, _)) => INTERFACE.settings.sender().send(new_settings),
                    Err(error) => warn!("Ignoring invalid settings: {}", error),
                },
                Ok(Message::Ping(data)) => {
                    tx.send_pong(data).await?;
                    tx.flush().await?;
                }
                Ok(Message::Close(_)) => break None,
                Ok(Message::Binary(_) | Message::Pong(_)) => {}
                Err(error) => break Some((error.code(), "Invalid message")),
            }
        };

        tx.close(close_reason).await
    }
}

async fn send<W: Write>(tx: &mut SocketTx<W>, update: Update) -> Result<(), W::Error> {
    tx.send_json(update).await?;
    tx.flush().await
}

#[embassy_executor::task(pool_size = MAX_SESSIONS)]
async fn serve(task_id: usize, stack: Stack<'static>) -> ! {
    let router = Router::new().route("/", get_service(INDEX_HTML)).route(
        "/ws",
        get(async |upgrade: WebSocketUpgrade| upgrade.on_upgrade(InterfaceSession)),
    );

    let mut tcp_rx_buffer = [0; 1024];
    let mut tcp_tx_buffer = [0; 1024];
    let mut http_buffer = [0; 2048];
    Server::new(&router, &CONFIG, &mut http_buffer)
        .listen_and_serve(task_id, stack, PORT, &mut tcp_rx_buffer, &mut tcp_tx_buffer)
        .await
        .into_never()
}

/// Serves the web interface on port 80 of `stack`. Panics if called twice.
pub fn spawn_web_server(spawner: Spawner, stack: Stack<'static>) {
    for task_id in 0..MAX_SESSIONS {
        spawner.spawn(unwrap!(serve(task_id, stack)));
    }
}
