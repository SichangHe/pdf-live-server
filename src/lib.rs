use anyhow::Result;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    http::header,
    response::{Html, IntoResponse, Response},
    routing::get,
    Extension, Router,
};
use clap::Parser;
use drop_this::DropResult;
use notify::RecommendedWatcher;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, Debouncer};
use pdf_reading::PdfReader;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use tokio::{
    fs::{metadata, read},
    net::TcpListener,
    sync::watch,
};
use tokio_gen_server::prelude::*;
use tower::ServiceBuilder;
use tracing::*;

mod pdf_reading;

/// Serve a PDF file live and reload the browser on changes.
#[derive(Parser, Debug)]
struct Args {
    /// Directory to watch for changes.
    #[arg(long, short = 'd', default_value = "./")]
    watch_dir: PathBuf,

    /// PDF file to serve. I also check its modified time to decide if changes occur.
    #[arg(long, short = 'f')]
    served_pdf: PathBuf,

    /// Address to bind the server.
    #[arg(long, short = 's', default_value = "127.0.0.1:3000")]
    socket_addr: SocketAddr,
}

type OptBytes = Option<Vec<u8>>;

pub async fn run() -> Result<()> {
    let args = Args::parse();
    let (tx, rx) = watch::channel::<OptBytes>(None);

    let pdf_reader = PdfReader {
        served_pdf: args.served_pdf.clone(),
        tx: tx.clone(),
        current_modified_time: None,
    };
    let (actor_handle, actor_ref) = pdf_reader.spawn();
    let _keep_debouncer_alive = start_watcher(args.watch_dir, actor_ref.clone())?;

    let app = Router::new()
        .route("/", get(serve_html))
        .route("/main.mjs", get(serve_js))
        .route("/__pdf_live_server_ws", get(ws_handler))
        .route("/served.pdf", get(serve_pdf))
        .layer(ServiceBuilder::new().layer(Extension(tx)))
        .layer(ServiceBuilder::new().layer(Extension(rx)));

    let listener = TcpListener::bind(args.socket_addr).await?;
    info!("Starting to listen on {}.", args.socket_addr);
    axum::serve(listener, app).await?;
    actor_ref.cancel();
    actor_handle.await?.exit_result?;
    Ok(())
}

async fn serve_html() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn serve_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        include_str!("main.mjs"),
    )
}
async fn serve_pdf(Extension(mut rx): Extension<watch::Receiver<OptBytes>>) -> impl IntoResponse {
    let maybe_bytes = rx.borrow().clone();
    let pdf_bytes = match maybe_bytes {
        pdf_bytes @ Some(_) => pdf_bytes,
        None => {
            warn!("No PDF bytes to serve for the route yet. Waiting.");
            await_pdf_bytes(&mut rx).await
        }
    }
    .unwrap_or(b"We must be shutting down.".into());
    ([(header::CONTENT_TYPE, "application/pdf")], pdf_bytes)
}

async fn await_pdf_bytes(rx: &mut watch::Receiver<OptBytes>) -> Option<Vec<u8>> {
    match rx.changed().await {
        Ok(_) => rx.borrow().clone(),
        _ => None,
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(tx): Extension<watch::Sender<OptBytes>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, tx.subscribe()))
}

async fn handle_socket(mut socket: WebSocket, mut rx: watch::Receiver<OptBytes>) {
    debug!("Connected via WebSocket.");
    while rx.changed().await.is_ok() {
        let msg = Message::Binary(rx.borrow().clone().expect("Updates are all `Some`."));
        if socket.send(msg).await.is_err() {
            break;
        }
        debug!("Sent message via WebSocket.");
    }
    info!("Closing WebSocket connection.");
}

pub const MS100: Duration = Duration::from_millis(100);

fn start_watcher(
    watch_dir: PathBuf,
    actor_ref: ActorRef<PdfReader>,
) -> notify::Result<Debouncer<RecommendedWatcher>> {
    let event_handler = move |event: notify::Result<Vec<DebouncedEvent>>| match event {
        Err(err) => error!(?err, "file watcher"),
        _ => actor_ref.blocking_cast(()).drop_result(),
    };

    let mut debouncer = new_debouncer(MS100, event_handler)?;
    debouncer
        .watcher()
        .watch(&watch_dir, notify::RecursiveMode::Recursive)?;
    Ok(debouncer)
}
