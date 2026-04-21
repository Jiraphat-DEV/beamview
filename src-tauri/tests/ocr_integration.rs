/// Integration test for the Apple Vision OCR wrapper.
///
/// Loads a fixture PNG containing "You cannot escape fate." (white text on dark
/// background, 800×120 px) and verifies that Vision Framework recognises it
/// with sufficient accuracy.
///
/// This test only runs on macOS; on other platforms it is skipped via
/// `#[cfg(target_os = "macos")]`.
#[cfg(target_os = "macos")]
#[test]
fn ocr_recognizes_subtitle_text() {
    use beamview_lib::translation::ocr::recognize_english;
    use image::GenericImageView;

    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/subtitle_sample.png");

    let img = image::open(&fixture).unwrap_or_else(|e| {
        panic!("failed to open fixture {}: {}", fixture.display(), e);
    });

    let rgba = img.to_rgba8();
    let (width, height) = img.dimensions();
    let raw = rgba.as_raw();

    let result = recognize_english(raw, width, height, None)
        .unwrap_or_else(|e| panic!("recognize_english returned error: {:?}", e));

    // Be lenient — OCR may vary punctuation, capitalisation, or spacing.
    let lower = result.to_lowercase();
    let stripped: String = lower
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();

    assert!(
        stripped.contains("escape fate") || stripped.contains("escape  fate"),
        "OCR output did not contain 'escape fate'.\nFull OCR result: {:?}",
        result
    );
}
