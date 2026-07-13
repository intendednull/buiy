//! `dooduel_mcp` — the stdio MCP agent bin (spec §7).
//!
//! One process = one seat. On start it opens a `WsClientTransport` to the Dooduel server
//! (the room the human host created), then runs the newline-delimited JSON-RPC 2.0 loop:
//! read one line from stdin, [`mcp::Server::handle`] it, write the response line to stdout
//! (flushed). stdout carries **only** JSON-RPC frames; all logs go to stderr, so an MCP
//! host reads a clean protocol stream (spec §7).
//!
//! Usage: `dooduel_mcp [--url ws://host:port]`. The server URL comes from `--url`, then
//! `DOODUEL_SERVER_URL`, else the default `ws://127.0.0.1:7878` (the server's default
//! bind). The agent takes its seat by calling the `join_room` tool.

use std::io::{BufRead as _, Write as _};

use dooduel_mcp::HeadlessClient;
use dooduel_mcp::mcp::Server;

fn main() {
    let url = server_url();
    eprintln!("dooduel_mcp: connecting to {url}");
    let client = match HeadlessClient::connect(&url) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("dooduel_mcp: failed to connect to {url}: {e}");
            std::process::exit(1);
        }
    };
    let mut server = Server::new(client);
    eprintln!("dooduel_mcp: ready — speaking JSON-RPC 2.0 on stdin/stdout");

    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut stdout = std::io::stdout();
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF — the host closed stdin.
            Ok(_) => {
                if let Some(response) = server.handle(&line) {
                    if writeln!(stdout, "{response}").is_err() {
                        break; // stdout closed
                    }
                    let _ = stdout.flush();
                }
            }
            Err(e) => {
                eprintln!("dooduel_mcp: stdin read error: {e}");
                break;
            }
        }
    }
}

/// Resolve the server URL: `--url <URL>` > `DOODUEL_SERVER_URL` > `ws://127.0.0.1:7878`.
fn server_url() -> String {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--url")
        && let Some(url) = args.get(i + 1)
    {
        return url.clone();
    }
    std::env::var("DOODUEL_SERVER_URL").unwrap_or_else(|_| "ws://127.0.0.1:7878".to_string())
}
