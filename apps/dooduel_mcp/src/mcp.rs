//! The hand-rolled stdio JSON-RPC 2.0 MCP surface (spec §7, decision 11).
//!
//! `rmcp` (the official SDK) hard-requires tokio — defeating the one-async-ecosystem
//! goal — and the tools-only surface an agent needs is tiny, so this is hand-rolled on
//! the existing `serde_json`: newline-delimited JSON-RPC 2.0 on stdin/stdout, stderr free
//! for logging. It advertises **only** the `tools` capability and exposes the 11
//! game-semantic tools (spec §7) 1:1 onto the protocol, each dispatched to a
//! [`crate::HeadlessClient`].
//!
//! [`Server::handle`] is the whole surface as a pure `&str → Option<String>` function
//! (`None` = a notification, which gets no response): unit-testable with no process spawn.
//! Malformed or unknown input yields a JSON-RPC error object — **never a panic** (no
//! `unwrap` on any stdin-reached path, spec §6.1).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use dooduel_core::transport::ClientTransport;

use crate::HeadlessClient;

/// The MCP protocol version this server speaks (the stable revision the tools-only
/// surface targets).
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// The stdio MCP server: a [`HeadlessClient`] behind the JSON-RPC dispatch. Generic over
/// the transport so the bin drives a real WebSocket while tests drive an in-process pair.
pub struct Server<T: ClientTransport> {
    client: HeadlessClient<T>,
    /// Set by `notifications/initialized` (advisory — the tools work regardless).
    initialized: bool,
}

impl<T: ClientTransport> Server<T> {
    /// Wrap a client in a fresh, un-initialized server.
    pub fn new(client: HeadlessClient<T>) -> Self {
        Self {
            client,
            initialized: false,
        }
    }

    /// The wrapped client (the bin pumps it between reads; tests seed it).
    pub fn client_mut(&mut self) -> &mut HeadlessClient<T> {
        &mut self.client
    }

    /// Handle one newline-delimited JSON-RPC message. Returns the response line, or `None`
    /// for a notification (no `id`) — which gets no response, even on error (JSON-RPC 2.0).
    pub fn handle(&mut self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        // A frame that is not valid JSON ⇒ Parse error with a null id (JSON-RPC 2.0 §5.1).
        let value: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => return Some(error_response(Value::Null, -32700, "Parse error")),
        };

        // A JSON-RPC request MUST be an object. An array (a batch — unsupported here) or a
        // bare scalar is an Invalid Request answered with a null id, never silently dropped
        // (W5-review minor 6).
        if !value.is_object() {
            return Some(error_response(Value::Null, -32600, "Invalid Request"));
        }

        let id = value.get("id").cloned();
        let is_notification = id.is_none();
        let method = value.get("method").and_then(Value::as_str);

        // A missing method is an Invalid Request (but a notification still gets no reply).
        let Some(method) = method else {
            return if is_notification {
                None
            } else {
                Some(error_response(
                    id.unwrap_or(Value::Null),
                    -32600,
                    "Invalid Request",
                ))
            };
        };

        // Notifications never get a response (even unknown ones).
        if is_notification {
            if method == "notifications/initialized" {
                self.initialized = true;
            }
            return None;
        }

        let id = id.unwrap_or(Value::Null);
        let params = value.get("params").cloned().unwrap_or(Value::Null);
        Some(match method {
            "initialize" => result_response(id, initialize_result()),
            "tools/list" => result_response(id, tools_list()),
            "tools/call" => self.tools_call(id, &params),
            "ping" => result_response(id, json!({})),
            _ => error_response(id, -32601, "Method not found"),
        })
    }

    /// Dispatch a `tools/call`: pump the transport so the reported state is fresh, then run
    /// the named tool. A missing tool name is a JSON-RPC `-32602`; a tool-level failure
    /// (unknown tool, bad arguments) is an MCP result with `isError: true` (the MCP
    /// convention — tool errors ride the result, not the protocol error channel).
    fn tools_call(&mut self, id: Value, params: &Value) -> String {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return error_response(id, -32602, "Invalid params: missing tool name");
        };
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        // Drain pending server events so get_state/get_canvas report the live room.
        self.client.pump();

        match self.dispatch_tool(name, &args) {
            Ok(content) => result_response(id, json!({ "content": content, "isError": false })),
            Err(message) => result_response(
                id,
                json!({ "content": [text_content(&message)], "isError": true }),
            ),
        }
    }

    /// Run one game-semantic tool (spec §7), returning its MCP content blocks. Every
    /// argument decode is fallible (stdin is untrusted — no panic); a bad argument is a
    /// tool error, surfaced to the agent as `isError` content.
    fn dispatch_tool(&mut self, name: &str, args: &Value) -> Result<Vec<Value>, String> {
        match name {
            "join_room" => {
                let room = str_arg(args, "room")?;
                let player = str_arg(args, "name")?;
                self.client.join(room, player, None);
                Ok(vec![text_content(
                    "Joining… call get_state to see the room once you are seated.",
                )])
            }
            "get_state" => Ok(vec![text_content(&self.client.state_report())]),
            "list_choices" => {
                let choices = self.client.word_choices();
                let text = if choices.is_empty() {
                    "No word choices right now (you are not the picking drawer).".to_string()
                } else {
                    let mut s = String::from("Your word choices (pick_word by index):\n");
                    for (i, w) in choices.iter().enumerate() {
                        s.push_str(&format!("- {i}: {w}\n"));
                    }
                    s
                };
                Ok(vec![text_content(&text)])
            }
            "pick_word" => {
                let index = usize_arg(args, "index")?;
                self.client.pick(index);
                Ok(vec![text_content(&format!(
                    "Picked word #{index}. You are the drawer — draw it with draw_stroke/fill."
                ))])
            }
            "guess" => {
                let text = str_arg(args, "text")?;
                self.client.guess(text.clone());
                Ok(vec![text_content(&format!("Guessed \"{text}\"."))])
            }
            "draw_stroke" => {
                let points = points_arg(args)?;
                let color = color_arg(args)?;
                let size = i32_arg(args, "size").unwrap_or(4).max(0);
                self.client.draw_stroke(points, color, size);
                Ok(vec![text_content("Stroke drawn.")])
            }
            "fill" => {
                let seed = seed_arg(args)?;
                let color = color_arg(args)?;
                self.client.fill(seed, color);
                Ok(vec![text_content("Filled.")])
            }
            "undo" => {
                self.client.undo();
                Ok(vec![text_content("Undid the last op.")])
            }
            "clear" => {
                self.client.clear();
                Ok(vec![text_content("Cleared the canvas.")])
            }
            "continue_turn" => {
                self.client.continue_turn();
                Ok(vec![text_content("Advancing to the next turn.")])
            }
            "get_canvas" => {
                let png = self.client.canvas_png();
                let data = STANDARD.encode(&png);
                Ok(vec![image_content(&data)])
            }
            other => Err(format!("unknown tool: {other}")),
        }
    }
}

// --- JSON-RPC response builders --------------------------------------------

/// The `initialize` result — advertise **only** the `tools` capability (spec §7).
fn initialize_result() -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "dooduel_mcp", "version": env!("CARGO_PKG_VERSION") },
    })
}

fn result_response(id: Value, result: Value) -> String {
    // Serializing our own well-formed value never fails; degrade to empty rather than panic.
    serde_json::to_string(&json!({ "jsonrpc": "2.0", "id": id, "result": result }))
        .unwrap_or_default()
}

fn error_response(id: Value, code: i64, message: &str) -> String {
    serde_json::to_string(
        &json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
    )
    .unwrap_or_default()
}

fn text_content(text: &str) -> Value {
    json!({ "type": "text", "text": text })
}

fn image_content(base64_png: &str) -> Value {
    json!({ "type": "image", "data": base64_png, "mimeType": "image/png" })
}

// --- Tool argument decoding (all fallible — stdin is untrusted) -------------

fn str_arg(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("missing or non-string argument `{key}`"))
}

fn i32_arg(args: &Value, key: &str) -> Result<i32, String> {
    args.get(key)
        .and_then(Value::as_i64)
        .map(|v| v as i32)
        .ok_or_else(|| format!("missing or non-integer argument `{key}`"))
}

fn usize_arg(args: &Value, key: &str) -> Result<usize, String> {
    let v = args
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing or non-integer argument `{key}`"))?;
    Ok(v as usize)
}

/// Parse `points`: an array of `[x, y]` integer pairs (the exact canvas coordinates).
fn points_arg(args: &Value) -> Result<Vec<(i32, i32)>, String> {
    let arr = args
        .get("points")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing array argument `points`".to_string())?;
    let mut out = Vec::with_capacity(arr.len());
    for p in arr {
        let pair = p
            .as_array()
            .ok_or_else(|| "each point must be an [x, y] pair".to_string())?;
        let x = pair
            .first()
            .and_then(Value::as_i64)
            .ok_or_else(|| "point x must be an integer".to_string())?;
        let y = pair
            .get(1)
            .and_then(Value::as_i64)
            .ok_or_else(|| "point y must be an integer".to_string())?;
        out.push((x as i32, y as i32));
    }
    if out.is_empty() {
        return Err("`points` must have at least one point".to_string());
    }
    Ok(out)
}

/// Parse `seed`: a single `[x, y]` integer pair (the fill seed).
fn seed_arg(args: &Value) -> Result<(i32, i32), String> {
    let pair = args
        .get("seed")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing `seed` [x, y] pair".to_string())?;
    let x = pair
        .first()
        .and_then(Value::as_i64)
        .ok_or_else(|| "seed x must be an integer".to_string())?;
    let y = pair
        .get(1)
        .and_then(Value::as_i64)
        .ok_or_else(|| "seed y must be an integer".to_string())?;
    Ok((x as i32, y as i32))
}

/// Parse `color`: an `[r, g, b]` or `[r, g, b, a]` byte array (alpha defaults to opaque).
fn color_arg(args: &Value) -> Result<[u8; 4], String> {
    let arr = args
        .get("color")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing `color` [r, g, b(, a)] array".to_string())?;
    if arr.len() != 3 && arr.len() != 4 {
        return Err("`color` must have 3 or 4 components".to_string());
    }
    let mut c = [0u8, 0, 0, 255];
    for (i, v) in arr.iter().enumerate() {
        let byte = v
            .as_u64()
            .filter(|n| *n <= 255)
            .ok_or_else(|| "each color component must be 0..=255".to_string())?;
        c[i] = byte as u8;
    }
    Ok(c)
}

// --- The tools/list catalog (spec §7 — the 11 game-semantic tools) ----------

/// The 11 tools, 1:1 onto the protocol (spec §7), each with a JSON-Schema `inputSchema`.
fn tools_list() -> Value {
    let no_args = json!({ "type": "object", "properties": {}, "additionalProperties": false });
    let color_schema = json!({
        "type": "array",
        "items": { "type": "integer", "minimum": 0, "maximum": 255 },
        "minItems": 3, "maxItems": 4,
        "description": "RGB or RGBA, 0..=255 each (e.g. the palette ink [20,20,24,255])."
    });
    let point_schema = json!({
        "type": "array",
        "items": { "type": "integer" },
        "minItems": 2, "maxItems": 2,
        "description": "An [x, y] canvas coordinate (0..720 × 0..450)."
    });

    json!({
        "tools": [
            {
                "name": "join_room",
                "description": "Join a Dooduel room by its invite code, taking a seat under the given name. Call get_state afterward to see the lobby/game.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "room": { "type": "string", "description": "The 6-character room code the host shared." },
                        "name": { "type": "string", "description": "Your display name (≤32 chars)." }
                    },
                    "required": ["room", "name"]
                }
            },
            {
                "name": "get_state",
                "description": "Your honest per-seat view of the room: phase, the (redacted) word row, the roster + scores, the recent chat, and the actions you can take right now. The secret word is never shown to a guesser before it is revealed.",
                "inputSchema": no_args
            },
            {
                "name": "list_choices",
                "description": "When you are the drawer in the picking phase, the three word choices you may pick from (by index).",
                "inputSchema": no_args
            },
            {
                "name": "pick_word",
                "description": "As the drawer, commit one of the offered word choices (from list_choices) and begin your turn.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "index": { "type": "integer", "minimum": 0, "description": "The word-choice index to draw." } },
                    "required": ["index"]
                }
            },
            {
                "name": "guess",
                "description": "Guess the word being drawn (only when you are not the drawer and have not already guessed it).",
                "inputSchema": {
                    "type": "object",
                    "properties": { "text": { "type": "string", "description": "Your guess (≤128 chars)." } },
                    "required": ["text"]
                }
            },
            {
                "name": "draw_stroke",
                "description": "As the drawer, stamp one stroke: a sequence of canvas points connected in order, at the given color and brush size.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "points": { "type": "array", "items": point_schema, "minItems": 1, "description": "The stroke's points, in order." },
                        "color": color_schema,
                        "size": { "type": "integer", "minimum": 0, "description": "The brush radius in pixels (default 4)." }
                    },
                    "required": ["points", "color"]
                }
            },
            {
                "name": "fill",
                "description": "As the drawer, flood-fill the region under a seed point with a color (the paint-bucket tool).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "seed": point_schema,
                        "color": color_schema
                    },
                    "required": ["seed", "color"]
                }
            },
            {
                "name": "undo",
                "description": "As the drawer, undo your last canvas operation.",
                "inputSchema": no_args
            },
            {
                "name": "clear",
                "description": "As the drawer, clear the whole canvas.",
                "inputSchema": no_args
            },
            {
                "name": "continue_turn",
                "description": "During the reveal, advance to the next turn.",
                "inputSchema": no_args
            },
            {
                "name": "get_canvas",
                "description": "The current drawing as a PNG image (the op log rasterized, plus any in-progress stroke). Use it to read what the drawer is drawing before you guess.",
                "inputSchema": no_args
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HeadlessClient;
    use dooduel_core::game::Phase;
    use dooduel_core::protocol::{PROTOCOL_VERSION, ReplicaPlayer, ServerEvent, WireAvatar};
    use dooduel_core::transport::{InProcClient, InProcessTransport};
    use std::time::Duration;

    /// A server over an in-process client (no socket, no process).
    fn server() -> Server<InProcClient> {
        let (_srv, mut clients) = InProcessTransport::new_pair(1);
        Server::new(HeadlessClient::new(clients.remove(0)))
    }

    fn parse(line: &str) -> Value {
        serde_json::from_str(line).expect("a response is valid JSON")
    }

    #[test]
    fn initialize_advertises_only_tools() {
        let mut s = server();
        let resp = s
            .handle(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
            .expect("initialize gets a response");
        let v = parse(&resp);
        assert_eq!(v["id"], json!(1), "the id is echoed");
        assert_eq!(v["result"]["protocolVersion"], json!(MCP_PROTOCOL_VERSION));
        assert!(
            v["result"]["capabilities"]["tools"].is_object(),
            "the tools capability is advertised"
        );
        // ONLY tools — no resources/prompts capabilities.
        assert!(v["result"]["capabilities"]["resources"].is_null());
        assert!(v["result"]["capabilities"]["prompts"].is_null());
        assert_eq!(v["result"]["serverInfo"]["name"], json!("dooduel_mcp"));
    }

    #[test]
    fn the_initialized_notification_gets_no_response() {
        let mut s = server();
        let resp = s.handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        assert_eq!(resp, None, "a notification gets no response");
        assert!(s.initialized, "but it is recorded");
    }

    #[test]
    fn tools_list_carries_all_eleven_tools() {
        let mut s = server();
        let resp = s
            .handle(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)
            .expect("tools/list gets a response");
        let v = parse(&resp);
        let tools = v["result"]["tools"].as_array().expect("a tools array");
        assert_eq!(tools.len(), 11, "all 11 game-semantic tools are listed");
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        for expected in [
            "join_room",
            "get_state",
            "list_choices",
            "pick_word",
            "guess",
            "draw_stroke",
            "fill",
            "undo",
            "clear",
            "continue_turn",
            "get_canvas",
        ] {
            assert!(names.contains(&expected), "tool `{expected}` is listed");
        }
        // Every tool carries an inputSchema (an object schema).
        for t in tools {
            assert_eq!(
                t["inputSchema"]["type"],
                json!("object"),
                "tool {t:?} has a schema"
            );
        }
    }

    #[test]
    fn a_tools_call_get_state_round_trips_to_the_report() {
        let mut s = server();
        // Seed a room via the fold so get_state has something to show.
        s.client_mut().apply(ServerEvent::Welcome {
            seat: 0,
            room_code: "ROOM01".to_string(),
            reconnect_token: "t".to_string(),
            protocol_version: PROTOCOL_VERSION,
        });
        s.client_mut().apply(ServerEvent::Roster {
            players: vec![ReplicaPlayer {
                name: "Ada".to_string(),
                avatar: WireAvatar::Default,
                connected: true,
                is_bot: false,
                score: 0,
                guessed: false,
            }],
            host: 0,
        });
        let resp = s
            .handle(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_state","arguments":{}}}"#)
            .expect("a response");
        let v = parse(&resp);
        assert_eq!(v["result"]["isError"], json!(false));
        let text = v["result"]["content"][0]["text"]
            .as_str()
            .expect("text content");
        assert!(text.contains("ROOM01"), "the report shows the room: {text}");
        assert!(text.contains("Ada"), "the report shows the roster");
    }

    #[test]
    fn get_canvas_returns_a_base64_png_image_block() {
        let mut s = server();
        s.client_mut().apply(ServerEvent::CanvasOpApplied {
            op: dooduel_core::protocol::CanvasOp::Stroke {
                id: 0,
                points: vec![(10, 10), (300, 200)],
                color: [10, 10, 12, 255],
                radius: 5,
            },
        });
        let resp = s
            .handle(r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_canvas","arguments":{}}}"#)
            .expect("a response");
        let v = parse(&resp);
        let block = &v["result"]["content"][0];
        assert_eq!(block["type"], json!("image"));
        assert_eq!(block["mimeType"], json!("image/png"));
        let data = block["data"].as_str().expect("base64 data");
        let png = STANDARD.decode(data).expect("valid base64");
        image::load_from_memory(&png).expect("a decodable PNG");
    }

    #[test]
    fn a_malformed_line_is_a_parse_error_with_a_null_id() {
        let mut s = server();
        let resp = s
            .handle("this is not json")
            .expect("a malformed line still answers");
        let v = parse(&resp);
        assert_eq!(
            v["id"],
            Value::Null,
            "a parse error echoes a null id (JSON-RPC 2.0)"
        );
        assert_eq!(v["error"]["code"], json!(-32700), "Parse error code");
    }

    #[test]
    fn a_non_object_frame_is_invalid_request_with_a_null_id() {
        // Arrays (batches — unsupported), and bare scalars, are Invalid Request (-32600)
        // with a null id — NOT silently dropped (W5-review minor 6).
        let mut s = server();
        for line in [
            "[1,2,3]",
            "42",
            "\"hello\"",
            r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#, // a JSON-RPC batch
        ] {
            let resp = s.handle(line).expect("a non-object frame still answers");
            let v = parse(&resp);
            assert_eq!(v["id"], Value::Null, "null id for {line}");
            assert_eq!(
                v["error"]["code"],
                json!(-32600),
                "Invalid Request for {line}"
            );
        }
    }

    #[test]
    fn an_unknown_method_is_method_not_found() {
        let mut s = server();
        let resp = s
            .handle(r#"{"jsonrpc":"2.0","id":9,"method":"no/such/method"}"#)
            .expect("a response");
        let v = parse(&resp);
        assert_eq!(v["id"], json!(9));
        assert_eq!(v["error"]["code"], json!(-32601));
    }

    #[test]
    fn ping_is_answered_with_an_empty_result() {
        let mut s = server();
        let resp = s
            .handle(r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#)
            .expect("ping answers");
        let v = parse(&resp);
        assert_eq!(v["id"], json!(7));
        assert_eq!(v["result"], json!({}));
    }

    #[test]
    fn an_unknown_tool_is_a_tool_error_not_a_protocol_error() {
        let mut s = server();
        let resp = s
            .handle(r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"fly_to_the_moon","arguments":{}}}"#)
            .expect("a response");
        let v = parse(&resp);
        // MCP convention: a tool failure is a result with isError, not a JSON-RPC error.
        assert!(v["error"].is_null(), "not a protocol error");
        assert_eq!(v["result"]["isError"], json!(true));
    }

    #[test]
    fn a_draw_stroke_call_parses_points_and_color() {
        let mut s = server();
        // Put the client in a state where it will send (drawer, drawing) — but the send
        // just queues on the in-process transport; we assert the call succeeds + parses.
        s.client_mut().apply(ServerEvent::PhaseChanged {
            phase: Phase::Drawing,
            drawer: Some(0),
            round: 1,
            total_rounds: 2,
            remaining: Duration::from_secs(80),
        });
        let resp = s
            .handle(r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"draw_stroke","arguments":{"points":[[10,10],[20,20]],"color":[20,20,24,255],"size":4}}}"#)
            .expect("a response");
        let v = parse(&resp);
        assert_eq!(
            v["result"]["isError"],
            json!(false),
            "a well-formed draw_stroke succeeds"
        );

        // A malformed color is a tool error (never a panic).
        let bad = s
            .handle(r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"draw_stroke","arguments":{"points":[[10,10]],"color":[999,0,0]}}}"#)
            .expect("a response");
        assert_eq!(parse(&bad)["result"]["isError"], json!(true));
    }
}
