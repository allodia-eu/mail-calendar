use super::draft_plain;

#[test]
fn bold_and_italic_markers_are_taken_off() {
    assert_eq!(
        draft_plain("It is **ready** and *tested*."),
        "It is ready and tested."
    );
}

#[test]
fn list_lines_read_as_the_composer_writes_them() {
    assert_eq!(
        draft_plain("Points:\n- one\n* **two**: done\n• three\n1) first\n2. second"),
        "Points:\n- one\n- two: done\n- three\n1. first\n2. second"
    );
}

#[test]
fn a_heading_loses_its_hashes() {
    assert_eq!(
        draft_plain("## Next steps\nCall me."),
        "Next steps\nCall me."
    );
}

#[test]
fn what_the_editor_leaves_as_text_is_left_as_written() {
    let text = "2 * 3 is **six\nsnake_case and *emphasis *\n#hashtag";
    assert_eq!(draft_plain(text), text);
}
