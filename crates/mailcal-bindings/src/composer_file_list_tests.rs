use mailcal_app::CompositionId;

use super::ComposerFileLists;
use crate::composer_files::ComposerFileAttachment;

fn composition(id: &str) -> CompositionId {
    CompositionId::new(id.to_owned()).expect("id")
}

fn file(path: &str, name: &str) -> ComposerFileAttachment {
    ComposerFileAttachment {
        path: path.to_owned(),
        file_name: name.to_owned(),
        media_type: String::new(),
    }
}

fn names(lists: &ComposerFileLists, id: &CompositionId) -> Vec<String> {
    lists
        .read(id)
        .tiles
        .into_iter()
        .map(|tile| tile.file_name)
        .collect()
}

#[test]
fn what_a_composer_opens_holding_is_listed_and_counts_as_unchanged() {
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    lists.open(
        &c,
        vec![
            file("/staged/a.pdf", "a.pdf"),
            file("/staged/b.txt", "b.txt"),
        ],
    );
    let files = lists.read(&c);
    assert_eq!(names(&lists, &c), ["a.pdf", "b.txt"]);
    assert!(!files.changed);
}

#[test]
fn a_file_added_later_is_a_change_and_removing_it_is_not_one() {
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    lists.open(&c, vec![file("/staged/a.pdf", "a.pdf")]);
    let added = lists.add(&c, vec![file("/picked/c.png", "c.png")]);
    assert!(added.changed);
    let id = added.tiles[1].id.clone();
    assert!(!lists.remove(&c, &id).changed);
}

#[test]
fn removing_a_file_the_composer_opened_with_is_a_change() {
    // A stored draft's own file taken off: the next save takes it off the server too, which the
    // user did, so leaving the composer must keep the draft.
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    lists.open(&c, vec![file("/staged/a.pdf", "a.pdf")]);
    let id = lists.read(&c).tiles[0].id.clone();
    let after = lists.remove(&c, &id);
    assert!(after.tiles.is_empty());
    assert!(after.changed);
}

#[test]
fn the_same_file_twice_is_two_tiles_with_their_own_ids() {
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    let files = lists.add(
        &c,
        vec![file("/p/a.pdf", "a.pdf"), file("/p/a.pdf", "a.pdf")],
    );
    assert_eq!(files.tiles.len(), 2);
    assert_ne!(files.tiles[0].id, files.tiles[1].id);
    let first = files.tiles[0].id.clone();
    assert_eq!(lists.remove(&c, &first).tiles.len(), 1);
}

#[test]
fn an_id_from_another_list_or_none_at_all_removes_nothing() {
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    lists.add(&c, vec![file("/p/a.pdf", "a.pdf")]);
    for stranger in ["file-9", "file-", "a.pdf", ""] {
        assert_eq!(lists.remove(&c, stranger).tiles.len(), 1, "{stranger:?}");
    }
}

#[test]
fn the_name_shown_is_the_one_the_message_carries() {
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    lists.add(
        &c,
        vec![file("/p/report.pdf", ""), file("/p/x", "../../etc/passwd")],
    );
    let shown = names(&lists, &c);
    assert_eq!(shown[0], "report.pdf");
    assert!(!shown[1].contains('/'), "{:?}", shown[1]);
    let sent: Vec<String> = lists
        .attachments(&c)
        .into_iter()
        .map(|file| file.file_name)
        .collect();
    assert_eq!(sent, shown);
}

#[test]
fn a_readable_file_shows_its_size_and_an_unreadable_one_none() {
    let path =
        std::env::temp_dir().join(format!("mailcal-composer-file-{}.txt", std::process::id()));
    std::fs::write(&path, b"forty-one bytes of a note, give or take..").expect("write");
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    let files = lists.add(
        &c,
        vec![
            file(path.to_str().expect("utf-8"), "note.txt"),
            file("/no/such/file.txt", "file.txt"),
        ],
    );
    std::fs::remove_file(&path).expect("remove");
    assert_eq!(files.tiles[0].size, Some(41));
    assert_eq!(files.tiles[1].size, None);
}

#[test]
fn each_composition_keeps_its_own_list_and_a_forgotten_one_holds_none() {
    let lists = ComposerFileLists::default();
    let (one, two) = (composition("c1"), composition("c2"));
    lists.add(&one, vec![file("/p/a.pdf", "a.pdf")]);
    lists.add(&two, vec![file("/p/b.pdf", "b.pdf")]);
    assert_eq!(names(&lists, &one), ["a.pdf"]);
    lists.forget(&one);
    assert!(lists.read(&one).tiles.is_empty());
    assert!(lists.attachments(&one).is_empty());
    assert_eq!(names(&lists, &two), ["b.pdf"]);
}

#[test]
fn every_tile_can_be_removed() {
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    let files = lists.add(&c, vec![file("/p/a.pdf", "a.pdf")]);
    assert!(files.tiles[0].remove.enabled);
    assert!(!files.tiles[0].remove.running);
}

#[test]
fn a_request_read_again_seeds_only_a_list_that_is_not_there_yet() {
    // An Outbox edit's opening is pulled until the host dismisses it; a second pull after the
    // user added a file must not put the list back to what the message was withdrawn with.
    let lists = ComposerFileLists::default();
    let c = composition("edit");
    lists.open_if_absent(&c, vec![file("/staged/a.pdf", "a.pdf")]);
    lists.add(&c, vec![file("/picked/b.pdf", "b.pdf")]);
    lists.open_if_absent(&c, vec![file("/staged/a.pdf", "a.pdf")]);
    assert_eq!(names(&lists, &c), ["a.pdf", "b.pdf"]);
    assert!(
        lists.read(&c).changed,
        "the staged file is what it opened with"
    );
}
