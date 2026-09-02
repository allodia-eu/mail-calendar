use crate::{safe_file_name, safe_media_type};

fn name(suggested: &str, path: &str) -> String {
    safe_file_name(suggested, path)
}

#[test]
fn the_suggested_name_wins_over_the_staged_path() {
    // A host that staged the bytes into a temporary file knows the display name separately, and
    // the temporary name is never the one the user recognises.
    assert_eq!(
        name("Holiday photo.jpg", "/data/cache/stage-8fa21c.tmp"),
        "Holiday photo.jpg"
    );
}

#[test]
fn an_absent_suggestion_falls_back_to_the_paths_last_component() {
    assert_eq!(name("", "/home/ada/report.pdf"), "report.pdf");
    assert_eq!(name("   ", "/home/ada/report.pdf"), "report.pdf");
}

#[test]
fn a_name_that_is_a_path_keeps_only_its_final_component() {
    // The gate against an attachment naming its way out of wherever the recipient saves it.
    // Both separators, because a name shared from Windows reaches this core unchanged.
    assert_eq!(name("../../etc/passwd", "/tmp/x"), "passwd");
    assert_eq!(name("photos\\holiday.jpg", "/tmp/x"), "holiday.jpg");
    assert_eq!(name("/", "/tmp/x"), "attachment");
}

#[test]
fn control_characters_cannot_break_out_of_the_header() {
    // CR and LF are the whole point: either would end the Content-Disposition line and let the
    // rest of the name become a header of the sharing app's choosing.
    // The colon becomes `_` on its own account: it is reserved punctuation, so what is left
    // cannot read as a header even to a careless eye.
    assert_eq!(
        name("invoice\r\nBcc: snoop@evil.test.pdf", "/tmp/x"),
        "invoiceBcc_ snoop@evil.test.pdf"
    );
    assert_eq!(name("a\u{0}b.txt", "/tmp/x"), "ab.txt");
}

#[test]
fn a_right_to_left_override_cannot_disguise_the_extension() {
    // Rendered, "holiday\u{202E}gpj.exe" reads as "holidayexe.jpg": the oldest attachment trick
    // there is. Dropping the override leaves the name reading in the order its bytes are in.
    assert_eq!(name("holiday\u{202E}gpj.exe", "/tmp/x"), "holidaygpj.exe");
    assert_eq!(name("a\u{2066}b\u{2069}.pdf", "/tmp/x"), "ab.pdf");
}

#[test]
fn reserved_punctuation_becomes_an_underscore() {
    assert_eq!(name("q1:q2*q3?.txt", "/tmp/x"), "q1_q2_q3_.txt");
}

#[test]
fn an_empty_or_all_stripped_name_becomes_attachment() {
    for candidate in ["", "...", "   ", "___", "\u{202E}"] {
        assert_eq!(name(candidate, ""), "attachment", "for {candidate:?}");
    }
}

#[test]
fn a_long_name_is_truncated_through_the_stem_so_the_extension_survives() {
    // The extension decides which application opens the file, so it is the last thing to lose.
    let long = format!("{}.pdf", "a".repeat(400));
    let truncated = name(&long, "/tmp/x");
    assert!(truncated.len() <= 200, "got {} bytes", truncated.len());
    assert_eq!(truncated.rsplit_once('.'), Some((&*"a".repeat(196), "pdf")));
    assert!(truncated.starts_with("aaaa"));
}

#[test]
fn truncation_never_splits_a_character_in_half() {
    // Every char here is three bytes, so a byte-count cut lands mid-character unless the
    // truncation walks back to a boundary. A panic is the failure this asserts against.
    let long = format!("{}.pdf", "日".repeat(200));
    let truncated = name(&long, "/tmp/x");
    assert!(truncated.len() <= 200);
    assert_eq!(truncated.rsplit_once('.').map(|(_, ext)| ext), Some("pdf"));
}

#[test]
fn a_long_name_with_no_usable_extension_is_simply_cut() {
    // A dot far from the end is part of the name, not an extension, so nothing is preserved
    // past the cut.
    let long = format!("{}.{}", "a".repeat(300), "b".repeat(300));
    assert_eq!(name(&long, "/tmp/x").len(), 200);
}

#[test]
fn a_well_formed_declared_media_type_is_kept_and_lowercased() {
    assert_eq!(
        safe_media_type("application/pdf", "report.pdf"),
        "application/pdf"
    );
    assert_eq!(safe_media_type("image/SVG+xml", "a.svg"), "image/svg+xml");
}

#[test]
fn parameters_are_dropped_rather_than_costing_the_type() {
    // A charset never has to survive for the part to be readable, and dropping it is better
    // than falling back to octet-stream over a semicolon.
    assert_eq!(
        safe_media_type("text/plain; charset=utf-8", "notes.txt"),
        "text/plain"
    );
}

#[test]
fn a_wildcard_is_not_a_media_type() {
    // What Android hands a share target that accepts anything. Answering the extension is
    // strictly more informative than repeating the wildcard onto the part.
    assert_eq!(safe_media_type("*/*", "report.pdf"), "application/pdf");
    assert_eq!(safe_media_type("image/*", "photo.png"), "image/png");
}

#[test]
fn a_malformed_declared_type_falls_back_to_the_extension() {
    for bad in ["/", "a/", "/b", "a/b/c", "bad media", "", "text"] {
        assert_eq!(
            safe_media_type(bad, "report.pdf"),
            "application/pdf",
            "expected the extension to answer for {bad:?}"
        );
    }
}

#[test]
fn an_unknown_extension_and_no_declared_type_is_octet_stream() {
    // Always a truthful answer: it says only "bytes".
    assert_eq!(
        safe_media_type("", "archive.qqq"),
        "application/octet-stream"
    );
    assert_eq!(
        safe_media_type("", "noextension"),
        "application/octet-stream"
    );
}

#[test]
fn the_extension_table_is_matched_case_insensitively() {
    assert_eq!(safe_media_type("", "PHOTO.JPG"), "image/jpeg");
    assert_eq!(safe_media_type("", "Invite.ICS"), "text/calendar");
}
