use super::{
    StoredWritingStyle, WritingStyleId, WritingStyles, load_writing_styles, save_writing_styles,
    writing_styles_path,
};

fn id(value: &str) -> WritingStyleId {
    WritingStyleId::new(value).unwrap()
}

fn style(name: &str) -> StoredWritingStyle {
    StoredWritingStyle {
        name: name.to_owned(),
        learned_from: "anna@example.eu".to_owned(),
        guide_json: r#"{"schema_version":1,"notes":"x = \"y\"\n"}"#.to_owned(),
        exemplars_json: r#"{"schema_version":1,"languages":{"en":["Hi"]}}"#.to_owned(),
        revealed: true,
    }
}

#[test]
fn a_library_round_trips_with_its_order_and_its_json_intact() {
    let dir = tempfile::tempdir().unwrap();
    let path = writing_styles_path(dir.path());
    let mut library = WritingStyles::default();
    library.insert(id("zzz"), style("Work"));
    library.insert(id("aaa"), style("Personal"));
    save_writing_styles(&path, &library).unwrap();

    let loaded = load_writing_styles(&path);
    assert_eq!(loaded, library);
    let names: Vec<&str> = loaded
        .ordered()
        .iter()
        .map(|(_, style)| style.name.as_str())
        .collect();
    assert_eq!(names, ["Work", "Personal"]);
    assert_eq!(
        loaded.get(&id("zzz")).unwrap().guide_json,
        style("x").guide_json,
        "the JSON comes back byte for byte, quotes and newlines included"
    );
}

#[test]
fn a_missing_or_broken_file_is_an_empty_library() {
    let dir = tempfile::tempdir().unwrap();
    let path = writing_styles_path(dir.path());
    assert_eq!(load_writing_styles(&path), WritingStyles::default());
    std::fs::write(&path, "not [toml").unwrap();
    assert_eq!(load_writing_styles(&path), WritingStyles::default());
}

#[test]
fn an_entry_missing_from_the_order_still_appears_and_a_bad_id_is_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let path = writing_styles_path(dir.path());
    let body = "order = [\"gone\"]\n\n[entries.kept]\nname = \"Kept\"\nguide_json = \"{}\"\n\n\
                [entries.\"  \"]\nname = \"Blank\"\nguide_json = \"{}\"\n";
    std::fs::write(&path, body).unwrap();
    let loaded = load_writing_styles(&path);
    let names: Vec<&str> = loaded
        .ordered()
        .iter()
        .map(|(_, style)| style.name.as_str())
        .collect();
    assert_eq!(names, ["Kept"]);
    assert!(!loaded.get(&id("kept")).unwrap().revealed);
}

#[test]
fn removing_a_style_takes_its_place_in_the_order() {
    let mut library = WritingStyles::default();
    library.insert(id("a"), style("A"));
    library.insert(id("a"), style("A again"));
    assert_eq!(library.order.len(), 1, "a replacement keeps its place");
    assert!(library.remove(&id("a")));
    assert!(!library.remove(&id("a")));
    assert_eq!(library.order, []);
}

#[test]
fn a_blank_or_control_id_is_refused() {
    assert!(WritingStyleId::new("   ").is_none());
    assert!(WritingStyleId::new("a\nb").is_none());
}

#[test]
fn a_style_prints_no_words() {
    let printed = format!("{:?}", style("Secret name"));
    assert!(!printed.contains("Secret"), "{printed}");
    assert!(!printed.contains("Hi"), "{printed}");
}
