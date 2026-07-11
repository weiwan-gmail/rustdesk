//! Headless controller API daemon.
//!
//! Exposes HTTP REST + WebSocket endpoints so an external agent can connect to
//! remotes, fetch decoded screen frames, inject input, and read/write clipboard.

mod auth;
mod clipboard;
mod frame_store;
mod handler;
mod input;
mod routes;
mod session_mgr;
mod ws;

pub use frame_store::{FrameStore, LatestFrame};
pub use handler::{HeadlessHandler, SessionState, SessionStatus};
pub use session_mgr::{HeadlessSession, SessionManager};

use hbb_common::log;
use std::sync::Arc;

/// Blocking entry used by `core_main` for `--api-server`.
pub fn run(bind: String, token: String) {
    log::info!("starting headless api-server on {bind}");
    let rt = match hbb_common::tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            log::error!("failed to create tokio runtime for api-server: {e}");
            eprintln!("failed to create tokio runtime for api-server: {e}");
            return;
        }
    };
    rt.block_on(async move {
        if let Err(e) = run_async(bind, token).await {
            log::error!("api-server exited with error: {e}");
            eprintln!("api-server exited with error: {e}");
        }
    });
}

async fn run_async(bind: String, token: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
    let sessions = Arc::new(SessionManager::new());
    let frames = Arc::new(FrameStore::new());
    let state = Arc::new(AppState {
        sessions: sessions.clone(),
        frames: frames.clone(),
        token,
    });

    let app = routes::router(state);

    let listener = hbb_common::tokio::net::TcpListener::bind(&bind).await?;
    log::info!("headless api-server listening on http://{bind}");
    println!("RustDesk headless API listening on http://{bind}");
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<SessionManager>,
    pub frames: Arc<FrameStore>,
    pub token: String,
}
