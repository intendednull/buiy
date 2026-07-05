# `dooduel_mcp` — the Dooduel agent client (MCP over stdio)

One process = one seat. The bin opens a WebSocket to a `dooduel_server`, then speaks
newline-delimited JSON-RPC 2.0 (the MCP stdio transport) on stdin/stdout — stdout
carries only protocol frames; logs go to stderr. An LLM agent playing Dooduel is an
ordinary protocol client, indistinguishable at the server from a human's GUI
(spec §7: `docs/specs/2026-07-04-dooduel-multiplayer-m1-design.md`).

The 11 tools: `join_room`, `get_state` (the honest per-seat view — the secret word is
never shown to a guesser pre-reveal), `list_choices`, `pick_word`, `guess`,
`draw_stroke`, `fill`, `undo`, `clear`, `continue_turn`, `get_canvas` (the drawing as
a PNG). Rooms are created and started by a host (the GUI); agents join by code.

Server URL: `--url ws://host:port` > `DOODUEL_SERVER_URL` > `ws://127.0.0.1:7878`.

## The M1 acceptance harness (plan W6.1)

The acceptance run = a human host in the GUI + N agent seats, one `dooduel_mcp`
process each, played to the podium over a live server. All commands are
fish-compatible; run from the repo root.

```sh
# 0. Build everything once (server + GUI + the agent bin):
cargo build -p dooduel_server -p dooduel -p dooduel_mcp

# 1. The server (terminal 1). Its stderr is the per-turn transcript — the
#    acceptance evidence stream; tee it to a file:
./target/debug/dooduel_server 2>| tee /tmp/dooduel-transcript.log
#    → prints `LISTENING port=7878`

# 2. The human host (terminal 2): set your name → "Create a room" → the live
#    lobby shows the server-issued 6-character code. Wait for the agents, then
#    "▶ Start game".
env RUST_MIN_STACK=33554432 cargo run -p dooduel

# 3. One agent seat (per agent; any working directory):
python3 apps/dooduel_mcp/examples/seat_driver.py \
    --bin ./target/debug/dooduel_mcp \
    --dir /tmp/dooduel-seat-1 --room <CODE> --name Priya
```

`seat_driver.py` (stdlib-only) bridges an LLM agent's file-based world onto the live
MCP stream — every action still traverses the real JSON-RPC → WebSocket →
authoritative-`Session` stack:

| File in `--dir` | Direction | Meaning |
|---|---|---|
| `state.md` | driver → agent | the seat's honest view (`get_state`), ~1/s |
| `canvas.png` | driver → agent | the current drawing (`get_canvas`), on change |
| `driver.log` | driver → agent | every command sent + tool result (append-only) |
| `commands.jsonl` | agent → driver | append one JSON object per line |

Command lines the agent may append:
`{"cmd":"pick","index":0}` · `{"cmd":"guess","text":"robot"}` ·
`{"cmd":"stroke","points":[[x,y],…],"color":[r,g,b],"size":6}` ·
`{"cmd":"fill","seed":[x,y],"color":[r,g,b]}` · `{"cmd":"undo"}` · `{"cmd":"clear"}`
· `{"cmd":"continue"}` · `{"cmd":"quit"}`.

Canvas coordinates are `0..720 × 0..450`; colors are RGB(A) `0..=255`. The drawer
sees its own strokes in `canvas.png` (the optimistic overlay); guessers watch the
drawing grow live. If `state.md` ever leads with a `⚠ CONNECTION LOST` banner, the
socket is gone — the driver should be restarted to rejoin.

Driving the bin RAW (no driver) is the same protocol the manual W5.4 session used:
send `initialize`, the `notifications/initialized` notification, then `tools/call`
frames, one JSON object per line.
