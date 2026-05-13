use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use lumen_common::HudSnapshot;
use tracing::{info, warn};

/// Cadence at which the publisher thread checks for a new snapshot and writes
/// it to the connected client. Independent of the simulator's tick rate — if
/// the sim ticks faster, intermediate snapshots are dropped (only latest
/// matters to a HUD).
const WRITE_INTERVAL: Duration = Duration::from_millis(20);

/// A handle the simulator uses to push the latest [`HudSnapshot`] to whatever
/// client is currently connected on the Unix socket. Cheaply cloneable; calls
/// to [`publish`](Self::publish) are non-blocking.
#[derive(Clone)]
pub struct Publisher {
    state: Arc<Mutex<Option<HudSnapshot>>>,
}

impl Publisher {
    pub fn start(socket_path: &Path) -> Result<Self> {
        // A stale socket from a previous run will cause bind() to fail with
        // EADDRINUSE. Cheaper than checking — just remove and re-bind.
        let _ = std::fs::remove_file(socket_path);
        let listener = UnixListener::bind(socket_path)
            .with_context(|| format!("binding UDS at {}", socket_path.display()))?;
        info!(socket = %socket_path.display(), "publisher listening");

        let state: Arc<Mutex<Option<HudSnapshot>>> = Arc::new(Mutex::new(None));
        let shared = state.clone();

        thread::Builder::new()
            .name("lumen-sim-publisher".into())
            .spawn(move || accept_loop(listener, shared))
            .context("spawning publisher thread")?;

        Ok(Self { state })
    }

    /// Replace the latest published snapshot. Non-blocking; if a client is
    /// slow, intermediate snapshots are simply overwritten before they're sent.
    pub fn publish(&self, snap: HudSnapshot) {
        *self.state.lock().expect("publisher mutex poisoned") = Some(snap);
    }
}

fn accept_loop(listener: UnixListener, state: Arc<Mutex<Option<HudSnapshot>>>) {
    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => serve_client(stream, &state),
            Err(e) => warn!(error = %e, "accept failed"),
        }
    }
}

fn serve_client(mut stream: UnixStream, state: &Arc<Mutex<Option<HudSnapshot>>>) {
    info!("client connected");
    let mut last_sent: Option<HudSnapshot> = None;
    loop {
        let current = *state.lock().expect("publisher mutex poisoned");
        if let Some(snap) = current
            && last_sent != Some(snap)
        {
            let line = match serde_json::to_string(&snap) {
                Ok(s) => s,
                Err(e) => {
                    warn!(error = %e, "snapshot serialization failed");
                    return;
                }
            };
            if writeln!(stream, "{line}").is_err() {
                break;
            }
            last_sent = Some(snap);
        }
        thread::sleep(WRITE_INTERVAL);
    }
    info!("client disconnected");
}
