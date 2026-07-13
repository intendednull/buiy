#!/usr/bin/env python3
"""seat_driver.py — run ONE Dooduel agent seat over the real MCP surface (M1 W6.1).

Spawns a `dooduel_mcp` process (one process = one seat, spec §7), performs the MCP
handshake, joins the room, and then bridges between an LLM agent's file-based world
and the live JSON-RPC stdio stream:

  <dir>/state.md        ← the seat's honest view (get_state), refreshed ~1/s
  <dir>/canvas.png      ← the current drawing (get_canvas), written when it changes
  <dir>/driver.log      ← every command sent + every tool result (append-only)
  <dir>/commands.jsonl  → the agent APPENDS one JSON object per line:
      {"cmd":"pick","index":0}
      {"cmd":"guess","text":"robot"}
      {"cmd":"stroke","points":[[x,y],...],"color":[r,g,b],"size":6}
      {"cmd":"fill","seed":[x,y],"color":[r,g,b]}
      {"cmd":"undo"} · {"cmd":"clear"} · {"cmd":"continue"} · {"cmd":"quit"}

Every action traverses the REAL production stack: this file only forwards
tools/call frames — the MCP server, the WebSocket transport, and the authoritative
Session do all the work. stdlib-only; no third-party imports.

Usage:
  python3 seat_driver.py --bin <path-to-dooduel_mcp> --dir <seat-dir> \
      --room <CODE> --name <Name> [--url ws://127.0.0.1:7878]
"""

import argparse
import base64
import json
import os
import queue
import subprocess
import sys
import threading
import time

POLL_SECS = 1.0


def atomic_write(path: str, data: bytes) -> None:
    tmp = path + ".tmp"
    with open(tmp, "wb") as f:
        f.write(data)
    os.replace(tmp, path)


class Mcp:
    """The JSON-RPC 2.0 stdio client around one dooduel_mcp process."""

    def __init__(self, bin_path: str, url: str, log):
        self.proc = subprocess.Popen(
            [bin_path, "--url", url],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
        )
        self.log = log
        self.next_id = 1
        self.responses: "queue.Queue[dict]" = queue.Queue()
        threading.Thread(target=self._reader, daemon=True).start()

    def _reader(self):
        assert self.proc.stdout is not None
        for line in self.proc.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                self.responses.put(json.loads(line))
            except json.JSONDecodeError:
                self.log(f"UNPARSEABLE stdout line: {line[:200]}")

    def _send(self, obj: dict) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write(json.dumps(obj) + "\n")
        self.proc.stdin.flush()

    def notify(self, method: str) -> None:
        self._send({"jsonrpc": "2.0", "method": method})

    def request(self, method: str, params=None, timeout=10.0) -> dict:
        rid = self.next_id
        self.next_id += 1
        frame = {"jsonrpc": "2.0", "id": rid, "method": method}
        if params is not None:
            frame["params"] = params
        self._send(frame)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                resp = self.responses.get(timeout=deadline - time.monotonic())
            except queue.Empty:
                break
            if resp.get("id") == rid:
                return resp
            self.log(f"skipping out-of-band frame: {json.dumps(resp)[:200]}")
        raise TimeoutError(f"no response to {method} (id {rid}) within {timeout}s")

    def call_tool(self, name: str, arguments=None) -> dict:
        params = {"name": name}
        if arguments:
            params["arguments"] = arguments
        return self.request("tools/call", params)

    def alive(self) -> bool:
        return self.proc.poll() is None


def tool_text(resp: dict) -> str:
    """The concatenated text content of a tools/call result (errors included)."""
    result = resp.get("result") or {}
    parts = []
    for block in result.get("content", []):
        if block.get("type") == "text":
            parts.append(block.get("text", ""))
    if resp.get("error"):
        parts.append(f"JSON-RPC error: {json.dumps(resp['error'])}")
    return "\n".join(parts)


def tool_png(resp: dict):
    """The first image content block of a tools/call result, decoded — or None."""
    result = resp.get("result") or {}
    for block in result.get("content", []):
        if block.get("type") == "image":
            return base64.b64decode(block.get("data", ""))
    return None


CMD_TO_TOOL = {
    "pick": ("pick_word", lambda c: {"index": c["index"]}),
    "guess": ("guess", lambda c: {"text": c["text"]}),
    "stroke": (
        "draw_stroke",
        lambda c: {
            "points": c["points"],
            "color": c.get("color", [20, 20, 24]),
            **({"size": c["size"]} if "size" in c else {}),
        },
    ),
    "fill": ("fill", lambda c: {"seed": c["seed"], "color": c.get("color", [20, 20, 24])}),
    "undo": ("undo", lambda c: None),
    "clear": ("clear", lambda c: None),
    "continue": ("continue_turn", lambda c: None),
}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", required=True)
    ap.add_argument("--dir", required=True)
    ap.add_argument("--room", required=True)
    ap.add_argument("--name", required=True)
    ap.add_argument("--url", default=os.environ.get("DOODUEL_SERVER_URL", "ws://127.0.0.1:7878"))
    args = ap.parse_args()

    os.makedirs(args.dir, exist_ok=True)
    log_file = open(os.path.join(args.dir, "driver.log"), "a", buffering=1)

    def log(msg: str) -> None:
        log_file.write(f"[{time.strftime('%H:%M:%S')}] {msg}\n")

    mcp = Mcp(args.bin, args.url, log)

    # The MCP handshake (spec §7): initialize → initialized → join.
    init = mcp.request(
        "initialize",
        {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "seat_driver", "version": "1"}},
    )
    log(f"initialize → {json.dumps(init.get('result', init))[:300]}")
    mcp.notify("notifications/initialized")
    joined = mcp.call_tool("join_room", {"room": args.room, "name": args.name})
    log(f"join_room({args.room}, {args.name}) → {tool_text(joined)[:300]}")
    if joined.get("error") or (joined.get("result") or {}).get("isError"):
        print(f"JOIN FAILED: {tool_text(joined)}", file=sys.stderr)
        return 1

    cmd_path = os.path.join(args.dir, "commands.jsonl")
    open(cmd_path, "a").close()  # the agent appends; we tail from the current end
    cmd_offset = 0
    last_png_len = -1

    while mcp.alive():
        # 1. Drain new agent commands (complete lines only).
        with open(cmd_path, "rb") as f:
            f.seek(cmd_offset)
            chunk = f.read()
        lines = chunk.split(b"\n")
        complete = lines[:-1]  # the final element is a partial line (or b"" after a trailing \n)
        for raw in complete:
            cmd_offset += len(raw) + 1  # consume the line + its newline
            line = raw.decode("utf-8", "replace").strip()
            if not line:
                continue
            try:
                cmd = json.loads(line)
            except json.JSONDecodeError as e:
                log(f"SKIP malformed command ({e}): {line[:200]}")
                continue
            kind = cmd.get("cmd")
            if kind == "quit":
                log("quit — shutting down")
                mcp.proc.terminate()
                return 0
            if kind not in CMD_TO_TOOL:
                log(f"SKIP unknown cmd {kind!r}")
                continue
            tool, to_args = CMD_TO_TOOL[kind]
            resp = mcp.call_tool(tool, to_args(cmd))
            log(f"{kind} → {tool_text(resp)[:400]}")

        # 2. Refresh the honest view + the canvas.
        state = mcp.call_tool("get_state")
        atomic_write(os.path.join(args.dir, "state.md"), tool_text(state).encode())
        png = tool_png(mcp.call_tool("get_canvas"))
        if png is not None and len(png) != last_png_len:
            atomic_write(os.path.join(args.dir, "canvas.png"), png)
            last_png_len = len(png)

        time.sleep(POLL_SECS)

    print("dooduel_mcp process exited", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
