//! The bundle's app icon (the prototype's
//! `test_app_icon_has_small_and_scalable_variants`).
//!
//! The prototype's `brand.app_icon()` drew the simplified mark (`icon-small.svg`) at 16–32 px and
//! the full icon above that, and its test asked for pixmaps of 16, 32 and 256 px. The Rust
//! application's icon is the bundle's `icons/icon.icns`, which Tauri copies into
//! `Contents/Resources`. `docs/brand.md` names `resources/brand/Sonara.icns` as the macOS icon,
//! whose 16–32 px entries `scripts/build_brand.py` renders from `icon-small.svg`; the bundle's
//! icons are those renders.

use std::fs;
use std::path::{Path, PathBuf};

/// The repository root, from the crate's manifest directory (`apps/sonara-desktop/src-tauri`).
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// The types of the ICNS entries in `bytes`: the `icns` magic and length, then each entry's
/// four-byte type and its length, which counts its own eight-byte header.
fn icns_types(bytes: &[u8]) -> Vec<String> {
    let be32 = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
    assert_eq!(&bytes[..4], b"icns", "the ICNS magic");
    assert_eq!(
        be32(4) as usize,
        bytes.len(),
        "the ICNS length is the file's"
    );
    let mut types = Vec::new();
    let mut at = 8;
    while at < bytes.len() {
        let kind = String::from_utf8_lossy(&bytes[at..at + 4]).into_owned();
        let len = be32(at + 4) as usize;
        assert!(
            len > 8 && at + len <= bytes.len(),
            "{kind}: its data fits the file"
        );
        types.push(kind);
        at += len;
    }
    types
}

#[test]
fn the_bundle_icon_has_the_brands_small_and_scalable_sizes() {
    let icons = root().join("apps/sonara-desktop/src-tauri/icons");
    let bundle_icon = fs::read(icons.join("icon.icns")).expect("icons/icon.icns");
    let kinds = icns_types(&bundle_icon);
    // The prototype's three sizes; each may come as any of the ICNS types of that size.
    for (px, types) in [
        (16, &["is32", "icp4", "ic04"][..]),
        (32, &["il32", "icp5", "ic05", "ic11"][..]),
        (256, &["ic08", "ic13"][..]),
    ] {
        assert!(
            types.iter().any(|t| kinds.iter().any(|k| k == t)),
            "no {px} px entry in {kinds:?}"
        );
    }
    // The brand's renders (docs/brand.md): the 16–32 px entries are the simplified mark. The
    // comparison runs while `resources/brand` exists, as the brand SVGs' test compares its copies.
    let brand = root().join("resources/brand");
    if let Ok(original) = fs::read(brand.join("Sonara.icns")) {
        assert_eq!(
            bundle_icon, original,
            "icon.icns is the brand's Sonara.icns"
        );
        for (bundle, render) in [
            ("32x32.png", "icon-32.png"),
            ("128x128.png", "icon-128.png"),
            ("128x128@2x.png", "icon-256.png"),
            ("icon.png", "icon-512.png"),
        ] {
            let ours = fs::read(icons.join(bundle)).expect(bundle);
            let theirs = fs::read(brand.join("png").join(render)).expect(render);
            assert_eq!(ours, theirs, "{bundle} is the brand's {render}");
        }
    }
}
