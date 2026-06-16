//! Task 2.3 self-test for the byte-exact `PackedInstance` hex check. Plain
//! `assert_eq!` (NOT a snapshot) so the round-trip cannot pass vacuously
//! (snapshots.md § Verification #3).

use buiy_core::render::instance::PackedInstance;
use buiy_verify::snapshot::instance_hex;

#[test]
fn hex_round_trips_bytes() {
    // `instance_hex(p)` → parse hex → `bytemuck::pod_read_unaligned` must
    // reconstruct the ORIGINAL `PackedInstance` bit-for-bit, proving the hex is
    // lossless and matches the GPU upload payload (52 B → 104 hex chars).
    let p = PackedInstance {
        rect_pos: [10.0, 20.0],
        rect_size: [100.0, 40.0],
        color: [0.25, 0.5, 0.75, 1.0],
        radius: 8.0,
        clip_min: [0.0, 0.0],
        clip_max: [200.0, 100.0],
    };

    let hex = instance_hex(&p);
    assert_eq!(hex.len(), 104, "52 bytes → 104 hex chars");

    // Parse the hex back into the 52 bytes.
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    assert_eq!(bytes.len(), std::mem::size_of::<PackedInstance>());

    let round: PackedInstance = bytemuck::pod_read_unaligned(&bytes);
    // PackedInstance has no PartialEq; compare its raw bytes (the GPU payload
    // identity that matters).
    assert_eq!(
        bytemuck::bytes_of(&round),
        bytemuck::bytes_of(&p),
        "hex round-trip must reconstruct the exact instance bytes"
    );
}

#[test]
fn hex_flips_on_a_packing_change() {
    // Teeth: a single-field change MUST flip the hex (so the snapshot has bite).
    let base = PackedInstance {
        rect_pos: [10.0, 20.0],
        rect_size: [100.0, 40.0],
        color: [1.0, 1.0, 1.0, 1.0],
        radius: 0.0,
        clip_min: [f32::NEG_INFINITY, f32::NEG_INFINITY],
        clip_max: [f32::INFINITY, f32::INFINITY],
    };
    let mut flipped = base;
    // The half-size sign bug `render_instance.rs` regression-tests: a negated
    // height must change the bytes.
    flipped.rect_size[1] = -flipped.rect_size[1];
    assert_ne!(
        instance_hex(&base),
        instance_hex(&flipped),
        "a negated height (the half-size sign bug) must flip the hex"
    );
}
