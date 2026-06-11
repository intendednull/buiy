//! The BuiyFont asset + loader (font-assets § 2): ttf/otf/ttc/otc only —
//! fontdb's native set — with the loader-output-is-always-sfnt invariant
//! (the named woff2 seam: adding woff2 later means a decompression
//! pre-pass HERE, touching neither registry nor FontSystem). Headless with
//! AssetPlugin (the atlas_register.rs precedent).

use buiy_core::text::{BuiyFont, BuiyFontLoader, BuiyFontLoaderError, sniff_sfnt};

const EMBEDDED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/fonts/FiraSans-Regular-latin.ttf"
);

#[test]
fn extensions_are_fontdbs_native_set() {
    use bevy::asset::AssetLoader;
    assert_eq!(BuiyFontLoader.extensions(), &["ttf", "otf", "ttc", "otc"]);
}

#[test]
fn sfnt_sniff_accepts_the_four_magics_and_rejects_the_rest() {
    // 0x00010000 (TrueType), OTTO (CFF), ttcf (collection), true (legacy).
    assert!(sniff_sfnt(&[0x00, 0x01, 0x00, 0x00, 0, 0]));
    assert!(sniff_sfnt(b"OTTO----"));
    assert!(sniff_sfnt(b"ttcf----"));
    assert!(sniff_sfnt(b"true----"));
    assert!(
        !sniff_sfnt(b"wOF2----"),
        "woff2 is the NAMED seam, not sfnt"
    );
    assert!(!sniff_sfnt(b"<svg"));
    assert!(!sniff_sfnt(&[]));
    let real = std::fs::read(EMBEDDED).unwrap();
    assert!(sniff_sfnt(&real));
}

#[test]
fn loader_loads_a_real_ttf_through_the_asset_server() {
    // End-to-end through bevy_asset: the loader registers for the
    // extensions and produces Arc'd bytes (zero-copy into
    // Source::Binary later).
    use bevy::asset::{AssetApp, AssetPlugin, AssetServer, Assets};
    use bevy::prelude::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin {
        // Serve the crate's assets dir so the embedded artifact doubles as
        // the load fixture (no second font committed for this test).
        file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
        ..Default::default()
    });
    app.init_asset::<BuiyFont>()
        .register_asset_loader(BuiyFontLoader);

    let handle: Handle<BuiyFont> = app
        .world()
        .resource::<AssetServer>()
        .load("fonts/FiraSans-Regular-latin.ttf");
    // Drive the async load to completion (bounded poll loop — the
    // condition-based-waiting discipline, no sleeps).
    for _ in 0..200 {
        app.update();
        if app
            .world()
            .resource::<Assets<BuiyFont>>()
            .get(&handle)
            .is_some()
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let font = app
        .world()
        .resource::<Assets<BuiyFont>>()
        .get(&handle)
        .expect("loaded within the poll budget");
    assert!(sniff_sfnt(&font.data), "loader output is always sfnt");
    assert_eq!(font.data.len(), std::fs::read(EMBEDDED).unwrap().len());
}

#[test]
fn loader_rejects_non_sfnt_bytes_through_the_asset_server() {
    // The enforcement point of loader-output-is-always-sfnt: a .ttf
    // extension routes to the loader, but non-sfnt bytes (a wOF2 magic —
    // the named seam) must fail the load with the NotSfnt error and never
    // materialize as a BuiyFont. Polls for the POSITIVE Failed signal
    // (condition-based waiting), not a never-loaded timeout.
    use bevy::asset::{AssetApp, AssetPlugin, AssetServer, Assets, LoadState};
    use bevy::prelude::*;

    // One-off fixture dir under the system tempdir (no tempfile dep):
    // woff2-magic bytes behind fontdb's native extension.
    let fixture_root =
        std::env::temp_dir().join(format!("buiy-font-asset-notsfnt-{}", std::process::id()));
    std::fs::create_dir_all(&fixture_root).unwrap();
    std::fs::write(fixture_root.join("not-a-font.ttf"), b"wOF2----").unwrap();

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin {
        // Absolute path: FileAssetReader joins it onto the base path, and
        // joining an absolute path replaces the base entirely.
        file_path: fixture_root.to_str().unwrap().to_owned(),
        ..Default::default()
    });
    app.init_asset::<BuiyFont>()
        .register_asset_loader(BuiyFontLoader);

    let handle: Handle<BuiyFont> = app.world().resource::<AssetServer>().load("not-a-font.ttf");
    let mut failed = None;
    for _ in 0..200 {
        app.update();
        if let Some(LoadState::Failed(err)) = app
            .world()
            .resource::<AssetServer>()
            .get_load_state(&handle)
        {
            failed = Some(err);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let err = failed.expect("non-sfnt load reports Failed within the poll budget");
    assert!(
        err.to_string().contains("not an sfnt font"),
        "failure is the sniff gate's NotSfnt, not IO: {err}"
    );
    assert!(
        app.world()
            .resource::<Assets<BuiyFont>>()
            .get(&handle)
            .is_none(),
        "rejected bytes never materialize as a BuiyFont"
    );
    std::fs::remove_dir_all(&fixture_root).ok();
}

#[test]
fn not_sfnt_error_names_the_woff2_seam_verbatim() {
    // The seam-naming message is load-bearing documentation (font-assets
    // § 9): pin it verbatim so a reword is a conscious act.
    assert_eq!(
        BuiyFontLoaderError::NotSfnt.to_string(),
        "not an sfnt font (ttf/otf/ttc/otc); woff2 needs the \
         font-assets § 9 decompression seam"
    );
}
