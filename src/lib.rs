    use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::{Html, Response},
    routing::get,
    Extension, Router,
};
use clap::Parser;
use drop_this::DropResult;
use notify::RecommendedWatcher;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, Debouncer};
use std::{
    fs::metadata,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use tokio::{net::TcpListener, sync::broadcast};
use tower::ServiceBuilder;
use tower_http::services::ServeFile;
use tracing::{debug, error, info};

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

pub async fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    let (tx, _) = broadcast::channel::<()>(100);

    let served_pdf = args.served_pdf.clone();
    let _keep_debouncer_alive = start_watcher(args.watch_dir, args.served_pdf, tx.clone())?;

    let app = Router::new()
        .route("/", get(serve_html))
        .route("/__pdf_live_server_ws", get(ws_handler))
        .nest_service("/served.pdf", ServeFile::new(served_pdf))
        .layer(ServiceBuilder::new().layer(Extension(tx)));

    let listener = TcpListener::bind(args.socket_addr).await?;
    info!("Starting to listen on {}.", args.socket_addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn serve_html() -> Html<&'static str> {
    Html(
        r#"
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>PDF Live Viewer</title>
<style>
    body, html { margin: 0; padding: 0; height: 100%; overflow: hidden; }
    iframe { border: none; width: 100%; height: 100%; }
</style>
</head>
<body>
<iframe id="pdfViewer"></iframe>
<script>
    // Append a timestamp to force reload
    const iframe = document.getElementById('pdfViewer');
    const timestamp = new Date().getTime();
    iframe.src = `served.pdf?cacheBust=${timestamp}`;

    const wsAddress = `ws://${location.host}/__pdf_live_server_ws`;
    const web_socket = new WebSocket(wsAddress);
    web_socket.onmessage = () => location.reload();
</script>
</body>
</html>
"#,
    )
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(tx): Extension<broadcast::Sender<()>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, tx.subscribe()))
}

async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<()>) {
    debug!("Connected via WebSocket.");
    while rx.recv().await.is_ok() {
        if socket.send(Message::Text("reload".into())).await.is_err() {
            break;
        }
        debug!("Sent message via WebSocket.");
    }
    info!("Closing WebSocket connection.");
}

pub const MS100: Duration = Duration::from_millis(100);

fn start_watcher(
    watch_dir: PathBuf,
    served_pdf: PathBuf,
    tx: broadcast::Sender<()>,
) -> notify::Result<Debouncer<RecommendedWatcher>> {
    let mut current_modified_time = modified_time(&served_pdf);
    let event_handler = move |event: notify::Result<Vec<DebouncedEvent>>| match event {
        Err(err) => error!(?err, "file watcher"),
        _ => {
            let new_modified_time = modified_time(&served_pdf);
            if new_modified_time != current_modified_time {
                current_modified_time = new_modified_time;
                tx.send(()).drop_result();
                info!(?served_pdf, "Notified about the change in modified time.");
            }
        }
    };

    let mut debouncer = new_debouncer(MS100, event_handler)?;
    debouncer
        .watcher()
        .watch(&watch_dir, notify::RecursiveMode::Recursive)?;
    Ok(debouncer)
}

fn modified_time(served_pdf: &Path) -> Option<SystemTime> {
    (|| metadata(served_pdf)?.modified())()
        .inspect_err(|err| {
            error!(?err, ?served_pdf, "getting modified time");
        })
        .ok()
}
