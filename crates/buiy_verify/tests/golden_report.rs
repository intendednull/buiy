//! Tier-5 triage-report self-test (Phase 3.8, verification-design `goldens.md`
//! § Verification #5). Pure-CPU, headless. Proves the HTML triage report is
//! **self-contained / offline-first**: every image is base64-inlined and the
//! file references no external URL or relative asset, so it opens straight from
//! a CI artifact with no network.

use buiy_core::render::golden::Dpr;
use buiy_verify::golden::{Backend, GoldenKey, TriageCard, TriageReport};
use buiy_verify::metric::{CompareOpts, FuzzBudget, compare};
use image::{Rgba, RgbaImage};

fn key() -> GoldenKey {
    GoldenKey {
        widget: "button".into(),
        state: "hover".into(),
        theme: "dark".into(),
        viewport: "sm".into(),
        backend: Backend::Lavapipe,
        dpr: Dpr::X2,
    }
}

fn png_bytes(img: &RgbaImage) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("encode PNG");
    buf.into_inner()
}

fn card() -> TriageCard {
    let baseline = RgbaImage::from_pixel(8, 8, Rgba([10, 120, 200, 255]));
    let mut actual = baseline.clone();
    actual.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
    let diff = compare(
        &actual,
        &baseline,
        &CompareOpts {
            emit_diff_image: true,
            ..CompareOpts::default()
        },
    );
    let diff_png = png_bytes(diff.diff_image.as_ref().expect("heatmap emitted"));
    TriageCard {
        key: key(),
        actual_png: png_bytes(&actual),
        baseline_png: png_bytes(&baseline),
        diff_png,
        diff,
        budget: FuzzBudget::EXACT,
    }
}

#[test]
fn report_is_self_contained() {
    let mut report = TriageReport::open_or_create(std::path::Path::new("/tmp/unused.html"));
    report.push(card());
    let html = report.render();

    // The PNGs are base64-inlined as data URIs (self-containment primitive).
    assert!(
        html.contains("data:image/png;base64,"),
        "PNGs must be base64-inlined as data URIs"
    );
    // Count the inlined images: 3 distinct PNGs (baseline, actual, diff) but the
    // baseline + actual appear twice each (side-by-side AND overlay) ⇒ 5 data
    // URIs total per card. At minimum every PNG is present.
    let n_data_uris = html.matches("data:image/png;base64,").count();
    assert!(
        n_data_uris >= 3,
        "expected at least 3 inlined PNGs, found {n_data_uris}"
    );

    // OFFLINE-FIRST: no network URL, no external/relative asset reference.
    assert!(
        !html.contains("http://") && !html.contains("https://"),
        "report must reference no external URL (offline-first)"
    );
    // No relative `src="./..."` or `href="..."` to an external file. The only
    // `src=` are the inlined data URIs.
    for needle in [
        "src=\"./",
        "src=\"/",
        "src=\"http",
        "href=\"http",
        "<script src",
    ] {
        assert!(
            !html.contains(needle),
            "report must not reference external asset `{needle}`"
        );
    }
    // Every `src="` attribute is a data URI (no externally-loaded image).
    for (i, _) in html.match_indices("src=\"") {
        let after = &html[i + 5..];
        assert!(
            after.starts_with("data:image/png;base64,"),
            "every img src must be an inlined data URI, found a non-data src"
        );
    }

    // The three triage views are present (side-by-side, overlay slider, diff
    // heatmap) and the key slug labels the card.
    assert!(
        html.contains(&key().slug()),
        "card is labeled by the key slug"
    );
    assert!(html.contains("diff heatmap"), "diff-heatmap view present");
    assert!(
        html.contains("type=\"range\""),
        "the JS opacity-slider overlay is present"
    );
    assert!(
        html.contains(".style.opacity"),
        "the overlay slider drives image opacity in pure JS (no framework)"
    );
}

#[test]
fn report_writes_a_self_contained_file() {
    // write() emits the same self-contained HTML to disk (the path the harness
    // points a reviewer at). Use a unique temp path.
    let path = std::env::temp_dir().join(format!(
        "buiy-golden-report-test/{}-report.html",
        std::process::id()
    ));
    let mut report = TriageReport::open_or_create(&path);
    report.push(card());
    report.write().expect("write report");

    let on_disk = std::fs::read_to_string(&path).expect("report readable");
    assert!(on_disk.contains("data:image/png;base64,"));
    assert!(on_disk.contains("<!doctype html>"));
    assert_eq!(report.path(), path, "path() returns the report location");
}

#[test]
fn multiple_cards_accumulate_with_unique_overlay_ids() {
    // Two failing cells in one run accumulate into one report, each with a
    // distinct overlay slider id so the sliders are independent.
    let mut report = TriageReport::open_or_create(std::path::Path::new("/tmp/unused2.html"));
    report.push(card());
    report.push(card());
    let html = report.render();
    assert!(html.contains("id=\"ov0\""), "first card overlay id");
    assert!(
        html.contains("id=\"ov1\""),
        "second card overlay id is unique"
    );
    assert!(
        html.contains("2 failing cell(s)"),
        "header counts both cards"
    );
}
