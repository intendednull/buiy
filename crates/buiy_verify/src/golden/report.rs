//! The self-contained, offline-first HTML triage report (`goldens.md` §
//! "Diff-PNG + self-contained HTML triage report"). On any golden `Fail` the
//! harness writes a diff-PNG and appends a card to a single
//! `target/buiy-goldens/report.html`, accumulating every failing cell from one
//! `cargo test` run. Each card embeds three views — side-by-side
//! expected|actual, a JS opacity-slider overlay, and the diff heatmap — with
//! all PNGs base64-inlined so the file references **no** external asset and
//! **no** network: it opens straight from a CI artifact (project ethos;
//! Skia-Gold §Borrow 6 reg-cli/x-img-diff-js, offline by construction).

use super::GoldenKey;
use crate::metric::{Diff, FuzzBudget};
use base64::Engine as _;

/// One failing golden cell, ready to render as an HTML card. The three PNG byte
/// vectors are inlined as base64 data URIs (self-containment) — `actual` is the
/// freshly captured frame, `baseline` is the *closest* stored positive
/// ([`GoldenOutcome::Fail::best`](super::GoldenOutcome::Fail), so the reviewer
/// compares against the nearest baseline, not an arbitrary one), and `diff` is
/// the [`Diff::diff_image`](crate::metric::Diff) heatmap.
pub struct TriageCard {
    /// The trace identity of the failing cell.
    pub key: GoldenKey,
    /// PNG bytes of the freshly captured frame.
    pub actual_png: Vec<u8>,
    /// PNG bytes of the closest stored positive.
    pub baseline_png: Vec<u8>,
    /// PNG bytes of the diff heatmap.
    pub diff_png: Vec<u8>,
    /// The metric outcome (counts + advisory MSSIM) for the card header.
    pub diff: Diff,
    /// The budget the cell was gated against (so the reviewer sees the bar it
    /// missed).
    pub budget: FuzzBudget,
}

/// A single HTML triage report accumulating one [`TriageCard`] per failing
/// cell. [`open_or_create`](Self::open_or_create) makes the report path
/// idempotent across a test run; [`write`](Self::write) emits one self-contained
/// file.
pub struct TriageReport {
    path: std::path::PathBuf,
    cards: Vec<TriageCard>,
}

impl TriageReport {
    /// Begin (or continue) a report at `path`. The cards accumulate in memory
    /// and [`write`](Self::write) re-emits the whole file, so multiple failing
    /// cells in one run land in one report. (We do not parse an existing HTML
    /// file back into cards — the driver holds the live `TriageReport` for the
    /// duration of a run; `open_or_create` exists so the path is the single
    /// source of truth.)
    pub fn open_or_create(path: &std::path::Path) -> Self {
        Self {
            path: path.to_path_buf(),
            cards: Vec::new(),
        }
    }

    /// The report's on-disk path.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Append a failing cell.
    pub fn push(&mut self, card: TriageCard) {
        self.cards.push(card);
    }

    /// Render the report and write it to [`path`](Self::path), creating parent
    /// directories. One self-contained HTML file: per card, a side-by-side
    /// expected|actual pair, a JS opacity-slider overlay, and the diff heatmap,
    /// all PNGs base64-inlined. No external assets, no network.
    pub fn write(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, self.render())
    }

    /// Render the full HTML document as a `String` (the testable core of
    /// [`write`](Self::write)).
    pub fn render(&self) -> String {
        let mut body = String::new();
        body.push_str(REPORT_HEAD);
        body.push_str(&format!(
            "<h1>Buiy golden triage — {} failing cell(s)</h1>\n",
            self.cards.len()
        ));
        for (i, card) in self.cards.iter().enumerate() {
            body.push_str(&card.render(i));
        }
        body.push_str(REPORT_TAIL);
        body
    }
}

impl TriageCard {
    /// Render one card. `idx` makes the overlay's slider/img element ids unique
    /// across cards in a single report.
    fn render(&self, idx: usize) -> String {
        let actual = data_uri(&self.actual_png);
        let baseline = data_uri(&self.baseline_png);
        let diff = data_uri(&self.diff_png);
        let mssim = self
            .diff
            .mssim
            .map(|s| format!("{s:.4}"))
            .unwrap_or_else(|| "—".into());
        format!(
            r#"<section class="card">
  <h2>{slug}</h2>
  <p class="meta">differing_pixels={dp} / {total} · max_channel_delta={mcd} · mssim={mssim}
     · budget=(Δ{bcd}, {bpx}px){saturated}</p>
  <div class="views">
    <figure><figcaption>expected (closest baseline)</figcaption><img alt="baseline" src="{baseline}"></figure>
    <figure><figcaption>actual</figcaption><img alt="actual" src="{actual}"></figure>
    <figure><figcaption>diff heatmap</figcaption><img alt="diff" src="{diff}"></figure>
  </div>
  <div class="overlay">
    <figcaption>overlay (drag to fade actual over baseline)</figcaption>
    <div class="stack">
      <img alt="overlay-baseline" src="{baseline}">
      <img id="ov{idx}" class="ov-top" alt="overlay-actual" src="{actual}">
    </div>
    <input type="range" min="0" max="100" value="50"
           oninput="document.getElementById('ov{idx}').style.opacity=this.value/100">
  </div>
</section>
"#,
            slug = html_escape(&self.key.slug()),
            dp = self.diff.differing_pixels,
            total = self.diff.total_pixels,
            mcd = self.diff.max_channel_delta,
            mssim = mssim,
            bcd = self.budget.max_channel_delta,
            bpx = self.budget.max_diff_pixels,
            saturated = if self.diff.saturated {
                " · SATURATED (dimension mismatch)"
            } else {
                ""
            },
        )
    }
}

/// Base64-inline PNG bytes as a `data:` URI — the self-containment primitive.
/// No external file, no network fetch.
fn data_uri(png: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
    format!("data:image/png;base64,{b64}")
}

/// Minimal HTML-escape for the slug text node (defense-in-depth; slugs are
/// already `[a-z0-9/_-]` so this is belt-and-braces).
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

const REPORT_HEAD: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Buiy golden triage</title>
<style>
  body { font: 14px/1.5 system-ui, sans-serif; margin: 1.5rem; background: #1a1a1a; color: #eee; }
  h1 { font-size: 1.3rem; }
  .card { border: 1px solid #444; border-radius: 8px; padding: 1rem; margin: 1rem 0; }
  .card h2 { font-size: 1rem; font-family: monospace; word-break: break-all; }
  .meta { font-family: monospace; color: #bbb; }
  .views { display: flex; gap: 1rem; flex-wrap: wrap; }
  figure { margin: 0; }
  figcaption { font-size: 0.8rem; color: #aaa; margin-bottom: 0.25rem; }
  img { image-rendering: pixelated; max-width: 320px; border: 1px solid #555; background:
        repeating-conic-gradient(#222 0% 25%, #2a2a2a 0% 50%) 50% / 16px 16px; }
  .stack { position: relative; display: inline-block; }
  .stack .ov-top { position: absolute; left: 0; top: 0; }
  input[type=range] { width: 320px; display: block; margin-top: 0.5rem; }
</style>
</head>
<body>
"#;

const REPORT_TAIL: &str = "</body>\n</html>\n";
