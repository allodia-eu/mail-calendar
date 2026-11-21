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
    let added = lists
        .add(&c, vec![file("/picked/c.png", "c.png")], None)
        .files;
    assert!(added.changed);
    let id = added.tiles[1].id.clone();
    assert!(!lists.remove(&c, &id, None).files.changed);
}

#[test]
fn removing_a_file_the_composer_opened_with_is_a_change() {
    // A stored draft's own file taken off: the next save takes it off the server too, which the
    // user did, so leaving the composer must keep the draft.
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    lists.open(&c, vec![file("/staged/a.pdf", "a.pdf")]);
    let id = lists.read(&c).tiles[0].id.clone();
    let after = lists.remove(&c, &id, None).files;
    assert!(after.tiles.is_empty());
    assert!(after.changed);
}

#[test]
fn the_same_file_twice_is_two_tiles_with_their_own_ids() {
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    let files = lists
        .add(
            &c,
            vec![file("/p/a.pdf", "a.pdf"), file("/p/a.pdf", "a.pdf")],
            None,
        )
        .files;
    assert_eq!(files.tiles.len(), 2);
    assert_ne!(files.tiles[0].id, files.tiles[1].id);
    let first = files.tiles[0].id.clone();
    assert_eq!(lists.remove(&c, &first, None).files.tiles.len(), 1);
}

#[test]
fn an_id_from_another_list_or_none_at_all_removes_nothing() {
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    lists.add(&c, vec![file("/p/a.pdf", "a.pdf")], None);
    for stranger in ["file-9", "file-", "a.pdf", ""] {
        assert_eq!(
            lists.remove(&c, stranger, None).files.tiles.len(),
            1,
            "{stranger:?}"
        );
    }
}

#[test]
fn the_name_shown_is_the_one_the_message_carries() {
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    lists.add(
        &c,
        vec![file("/p/report.pdf", ""), file("/p/x", "../../etc/passwd")],
        None,
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
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("note.txt");
    std::fs::write(&path, b"forty-one bytes of a note, give or take..").expect("write");
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    let files = lists
        .add(
            &c,
            vec![
                file(path.to_str().expect("utf-8"), "note.txt"),
                file("/no/such/file.txt", "file.txt"),
            ],
            None,
        )
        .files;
    std::fs::remove_file(&path).expect("remove");
    assert_eq!(files.tiles[0].size, Some(41));
    assert_eq!(files.tiles[1].size, None);
    // The summary adds up what could be read; a file it could not read adds nothing.
    assert_eq!(files.total_size, 41);
}

#[test]
fn a_picture_by_its_bytes_offers_a_preview_and_anything_else_none() {
    let dir = tempfile::tempdir().expect("dir");
    let picture = dir.path().join("shot.png");
    std::fs::write(&picture, b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR").expect("write");
    // Named like a picture, but the bytes are an SVG, which is script-capable: no preview.
    let liar = dir.path().join("logo.png");
    std::fs::write(&liar, b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>").expect("write");
    let paths = [&picture, &liar].map(|path| path.to_str().expect("utf-8").to_owned());
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    let files = lists
        .add(
            &c,
            vec![
                file(&paths[0], "shot.png"),
                file(&paths[1], "logo.png"),
                file("/no/such/photo.jpg", "photo.jpg"),
            ],
            None,
        )
        .files;
    drop(dir);
    assert_eq!(files.tiles[0].preview.as_deref(), Some(paths[0].as_str()));
    assert_eq!(files.tiles[1].preview, None);
    assert_eq!(files.tiles[2].preview, None);
}

#[test]
fn the_total_follows_a_file_taken_off() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("total.txt");
    std::fs::write(&path, [0u8; 100]).expect("write");
    let text = path.to_str().expect("utf-8");
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    let files = lists
        .add(&c, vec![file(text, "a.txt"), file(text, "b.txt")], None)
        .files;
    assert_eq!(files.total_size, 200);
    let after = lists.remove(&c, &files.tiles[0].id, None).files;
    std::fs::remove_file(&path).expect("remove");
    assert_eq!(after.total_size, 100);
    assert_eq!(lists.read(&composition("none")).total_size, 0);
}

#[test]
fn each_composition_keeps_its_own_list_and_a_forgotten_one_holds_none() {
    let lists = ComposerFileLists::default();
    let (one, two) = (composition("c1"), composition("c2"));
    lists.add(&one, vec![file("/p/a.pdf", "a.pdf")], None);
    lists.add(&two, vec![file("/p/b.pdf", "b.pdf")], None);
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
    let files = lists.add(&c, vec![file("/p/a.pdf", "a.pdf")], None).files;
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
    lists.add(&c, vec![file("/picked/b.pdf", "b.pdf")], None);
    lists.open_if_absent(&c, vec![file("/staged/a.pdf", "a.pdf")]);
    assert_eq!(names(&lists, &c), ["a.pdf", "b.pdf"]);
    assert!(
        lists.read(&c).changed,
        "the staged file is what it opened with"
    );
}

#[test]
fn a_remove_for_a_composition_that_has_gone_starts_no_list() {
    // A Remove pressed as the message was sent or discarded reaches a composition the core has
    // forgotten; it must not start one again, empty, for the rest of the process.
    let lists = ComposerFileLists::default();
    let c = composition("gone");
    let answer = lists.remove(&c, "file-0", None).files;
    assert!(answer.tiles.is_empty());
    assert!(!lists.lock().contains_key(&c));
}

#[test]
fn a_change_counts_the_files_added_since_a_mark_before_and_after_it() {
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    lists.open(&c, vec![file("/staged/a.pdf", "a.pdf")]);
    let mark = lists.mark(&c);
    let added = lists.add(&c, vec![file("/p/b.pdf", "b.pdf")], Some(mark));
    assert_eq!(added.added, Some((0, 1)));
    let opened = added.files.tiles[0].id.clone();
    assert_eq!(lists.remove(&c, &opened, Some(mark)).added, Some((1, 1)));
    assert_eq!(lists.add(&c, Vec::new(), None).added, None);
}

#[test]
fn an_addition_finished_after_a_mark_counts_as_one_made_at_once() {
    // A reply drafted while a film was still being copied: the film lands after the mark and is
    // counted as an addition, as a file added at once would be.
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    lists.open(&c, vec![file("/staged/a.pdf", "a.pdf")]);
    lists.begin_adding(&c);
    let mark = lists.mark(&c);
    let ended = lists.finish_adding(&c, vec![file("/p/film.mov", "film.mov")], Some(mark));
    assert_eq!(ended.added, Some((0, 1)));
    assert_eq!(lists.finish_adding(&c, Vec::new(), None).added, None);
}

fn attach(lists: &ComposerFileLists, id: &CompositionId) -> (bool, bool) {
    let attach = lists.read(id).attach;
    (attach.enabled, attach.running)
}

#[test]
fn attach_is_free_while_no_file_is_on_its_way() {
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    assert_eq!(attach(&lists, &c), (true, false));
    lists.open(&c, vec![file("/staged/a.pdf", "a.pdf")]);
    lists.add(&c, vec![file("/p/b.pdf", "b.pdf")], None);
    assert_eq!(attach(&lists, &c), (true, false));
}

#[test]
fn attach_runs_from_the_start_of_an_addition_until_its_files_are_in() {
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    lists.open(&c, Vec::new());
    let started = lists.begin_adding(&c);
    assert!(started.attach.running);
    assert!(!started.attach.enabled, "a running control is held");
    assert!(started.tiles.is_empty());
    assert!(!started.changed, "nothing has arrived yet");
    let finished = lists
        .finish_adding(&c, vec![file("/p/film.mov", "film.mov")], None)
        .files;
    assert_eq!(names(&lists, &c), ["film.mov"]);
    assert!(finished.changed);
    assert_eq!(attach(&lists, &c), (true, false));
}

#[test]
fn attach_runs_until_every_addition_under_way_has_ended() {
    // A drop while a pick is still being copied: the first to end must not stop the spinner the
    // other still needs.
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    lists.begin_adding(&c);
    lists.begin_adding(&c);
    lists.finish_adding(&c, vec![file("/p/a.pdf", "a.pdf")], None);
    assert_eq!(attach(&lists, &c), (false, true));
    lists.finish_adding(&c, vec![file("/p/b.pdf", "b.pdf")], None);
    assert_eq!(attach(&lists, &c), (true, false));
    assert_eq!(names(&lists, &c), ["a.pdf", "b.pdf"]);
}

#[test]
fn an_addition_that_brought_nothing_ends_without_a_change() {
    // Every chosen picture failed to copy: the addition still ends, and the list is as it was.
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    lists.open(&c, vec![file("/staged/a.pdf", "a.pdf")]);
    lists.begin_adding(&c);
    let ended = lists.finish_adding(&c, Vec::new(), None).files;
    assert_eq!(names(&lists, &c), ["a.pdf"]);
    assert!(!ended.changed);
    assert_eq!((ended.attach.enabled, ended.attach.running), (true, false));
}

#[test]
fn only_an_addition_that_was_started_ends_one() {
    // Files ready at once (a share at opening) go through `add`, which ends nothing; and an end
    // with none under way leaves the count at none rather than owing the next one.
    let lists = ComposerFileLists::default();
    let c = composition("c1");
    lists.begin_adding(&c);
    lists.add(&c, vec![file("/shared/a.pdf", "a.pdf")], None);
    assert_eq!(attach(&lists, &c), (false, true));
    lists.finish_adding(&c, Vec::new(), None);
    lists.finish_adding(&c, Vec::new(), None);
    lists.begin_adding(&c);
    assert_eq!(attach(&lists, &c), (false, true));
}

#[test]
fn an_addition_ending_after_its_composition_has_gone_starts_no_list() {
    // A film still being copied when the message was discarded: its files have nowhere to go,
    // and the composition must not be started again for the rest of the process.
    let lists = ComposerFileLists::default();
    let c = composition("gone");
    lists.open(&c, Vec::new());
    lists.begin_adding(&c);
    lists.forget(&c);
    let answer = lists
        .finish_adding(&c, vec![file("/p/film.mov", "film.mov")], None)
        .files;
    assert!(answer.tiles.is_empty());
    assert!(!lists.lock().contains_key(&c));
}
