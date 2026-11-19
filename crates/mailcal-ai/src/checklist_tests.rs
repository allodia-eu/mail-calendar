use super::{AnsweredTask, DraftTask, TaskKind, checklist, summary};

fn asked(kind: &str, text: &str) -> AnsweredTask {
    AnsweredTask {
        kind: kind.to_owned(),
        text: text.to_owned(),
    }
}

#[test]
fn the_reply_s_placeholders_come_first_then_what_the_model_listed() {
    let tasks = checklist(
        &["[date]".to_owned(), "[amount]".to_owned()],
        vec![
            asked("attach", "Attach the construction drawings"),
            asked("do", "Check the budget with Finance"),
        ],
    );
    assert_eq!(
        tasks,
        [
            DraftTask::new(TaskKind::FillIn, "[date]"),
            DraftTask::new(TaskKind::FillIn, "[amount]"),
            DraftTask::new(TaskKind::Attach, "Attach the construction drawings"),
            DraftTask::new(TaskKind::Do, "Check the budget with Finance"),
        ]
    );
}

#[test]
fn a_kind_the_model_made_up_is_a_thing_to_do_and_an_empty_item_is_dropped() {
    let tasks = checklist(
        &[],
        vec![asked("action", "Update the CRM"), asked("attach", "   ")],
    );
    assert_eq!(tasks, [DraftTask::new(TaskKind::Do, "Update the CRM")]);
}

#[test]
fn the_list_is_short_and_each_item_one_line() {
    let many = (0..10).map(|n| asked("do", &format!("Task {n}\nwith a second line")));
    let tasks = checklist(&[], many.collect());
    assert_eq!(tasks.len(), 6);
    assert_eq!(tasks[0].text, "Task 0 with a second line");

    let long = checklist(&[], vec![asked("do", &"x".repeat(300))]);
    assert_eq!(long[0].text.chars().count(), 161);
    assert!(long[0].text.ends_with('…'));
}

#[test]
fn a_summary_is_one_paragraph_and_bounded() {
    assert_eq!(
        summary("  Marc asks for the drawings\nof the low-rise.  "),
        "Marc asks for the drawings of the low-rise."
    );
    assert_eq!(summary(&"y".repeat(900)).chars().count(), 401);
}

#[test]
fn a_task_prints_its_kind_and_never_its_text() {
    let printed = format!(
        "{:?}",
        DraftTask::new(TaskKind::Attach, "the secret contract")
    );
    assert!(!printed.contains("secret"));
    assert!(printed.contains("Attach"));
}

#[test]
fn the_summary_and_the_model_s_items_start_with_a_capital() {
    assert_eq!(
        summary("marc asks for the drawings."),
        "Marc asks for the drawings."
    );
    let tasks = checklist(
        &["[tijdstip]".to_owned()],
        vec![asked("attach", "constructietekeningen meesturen")],
    );
    assert_eq!(tasks[0].text, "[tijdstip]");
    assert_eq!(tasks[1].text, "Constructietekeningen meesturen");
}
