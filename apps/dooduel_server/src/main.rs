//! `dooduel_server` — the authoritative networked Dooduel server (spec §6).
//!
//! One tokio-free `async_executor::Executor`, driven by the main thread via
//! `async_io::block_on(EX.run(..))`, runs every task cooperatively: the accept loop, one
//! [`wire::handle_conn`] per connection, and one [`room::room_task`] actor per room. Each
//! room actor **solely owns** its `dooduel_core::Session` — no mutex on game state, intake
//! order = mutation order = deterministic (spec §6.1). No Bevy/Buiy GUI stack: the server
//! depends on `dooduel_core` only (locked decision 5). `ws://` only in M1 (no TLS, §6.2).
//!
//! Usage: `dooduel_server [--port N]`. Precedence for the bind address:
//! `--port N` (binds `127.0.0.1:N`) > `DOODUEL_ADDR` (a full `host:port`, e.g.
//! `0.0.0.0:7878` for LAN) > the default `127.0.0.1:7878` (matches the GUI's default
//! `ws://127.0.0.1:7878`). `--port 0` binds an OS-chosen port. On bind the server prints
//! `LISTENING port=<n>` to stdout (flushed) — the e2e/smoke discovery line (spec §9.5).

use std::io::Write as _;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_executor::Executor;

use registry::Registry;

mod registry;
mod room;
mod util;
mod wire;

/// The single global executor every server task runs on (spec §6.1). Driven by the main
/// thread; tasks are cooperative and `Send`.
pub static EX: Executor<'static> = Executor::new();

fn main() {
    let addr = parse_addr();
    let registry = Registry::new();
    async_io::block_on(EX.run(async move {
        let listener = match async_net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("dooduel_server: failed to bind {addr}: {e}");
                std::process::exit(1);
            }
        };
        let local = listener
            .local_addr()
            .expect("a bound listener has a local address");
        // The discovery line (spec §9.5), FLUSHED so a piped stdout reader (the e2e /
        // smoke) sees it immediately rather than on a full block buffer.
        println!("LISTENING port={}", local.port());
        let _ = std::io::stdout().flush();
        eprintln!("dooduel_server: listening on {local} (ws://)");
        accept_loop(listener, registry).await;
    }));
}

/// Accept connections forever, spawning one [`wire::handle_conn`] per socket with a fresh
/// connection id (spec §6.1).
async fn accept_loop(listener: async_net::TcpListener, registry: Arc<Registry>) {
    let next_conn = AtomicU64::new(0);
    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                let conn = next_conn.fetch_add(1, Ordering::Relaxed);
                EX.spawn(wire::handle_conn(stream, peer, Arc::clone(&registry), conn))
                    .detach();
            }
            // A single accept error (e.g. a transient FD exhaustion) must not kill the
            // server — log and keep accepting.
            Err(e) => eprintln!("dooduel_server: accept error: {e}"),
        }
    }
}

/// Resolve the bind address (spec §6.2): `--port N` > `DOODUEL_ADDR` > `127.0.0.1:7878`.
fn parse_addr() -> SocketAddr {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--port")
        && let Some(port) = args.get(i + 1).and_then(|s| s.parse::<u16>().ok())
    {
        return SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    }
    if let Ok(addr) = std::env::var("DOODUEL_ADDR")
        && let Ok(sa) = addr.parse::<SocketAddr>()
    {
        return sa;
    }
    SocketAddr::from((Ipv4Addr::LOCALHOST, 7878))
}
