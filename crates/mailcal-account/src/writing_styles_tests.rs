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
        source: "account-1".to_owned(),
        guide_json: r#"{"schema_version":1,"notes":"x = \"y\"\n"}"#.to_owned(),
        exemplars_json: r#"{"schema_version":1,"languages":{"en":["Hi"]}}"#.to_owned(),
    }
}

#[test]
fn a_library_round_trips_with_its_order_and_its_json_intact() {
    let dir = std::env::temp_dir().join("mailcal-writing-styles-roundtrip-test");
    let _ = std::fs::remove_dir_all(&dir);
    let path = writing_styles_path(&dir);

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
    // The JSON bodies come back byte for byte, quotes and newlines included.
    assert_eq!(
        loaded.get(&id("zzz")).unwrap().guide_json,
        style("x").guide_json
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_or_broken_file_is_an_empty_library() {
    let dir = std::env::temp_dir().join("mailcal-writing-styles-broken-test");
    let _ = std::fs::remove_dir_all(&dir);
    let path = writing_styles_path(&dir);
    assert_eq!(load_writing_styles(&path), WritingStyles::default());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&path, "not [toml").unwrap();
    assert_eq!(load_writing_styles(&path), WritingStyles::default());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A hand-edited file that drops an id from `order` must not hide the style it names.
#[test]
fn an_entry_missing_from_the_order_still_appears() {
    let mut library = WritingStyles::default();
    library.insert(id("b"), style("B"));
    library.insert(id("a"), style("A"));
    library.order.clear();
    library.order.push(id("gone"));
    let ids: Vec<&str> = library
        .ordered()
        .iter()
        .map(|(id, _)| id.as_str())
        .collect();
    assert_eq!(ids, ["a", "b"]);
}

#[test]
fn remove_takes_the_entry_and_its_place() {
    let mut library = WritingStyles::default();
    library.insert(id("a"), style("A"));
    assert!(library.remove(&id("a")));
    assert!(!library.remove(&id("a")));
    assert!(library.order.is_empty());
}

#[test]
fn an_id_must_be_printable_and_not_blank() {
    assert!(WritingStyleId::new(" ").is_none());
    assert!(WritingStyleId::new("a\nb").is_none());
    assert_eq!(WritingStyleId::new("abc").unwrap().as_str(), "abc");
}

/// The passages are the person's own mail: nothing of them, or of the guide, reaches a log.
#[test]
fn debug_output_carries_lengths_not_text() {
    let printed = format!("{:?}", style("Work"));
    assert!(!printed.contains("Hi"), "{printed}");
    assert!(!printed.contains("Work"), "{printed}");
    assert!(printed.contains("exemplars_len"));
}
