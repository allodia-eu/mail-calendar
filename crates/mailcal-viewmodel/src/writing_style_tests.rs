use super::{HabitFrequency, habit_rows};

fn rows(shares: &[u8]) -> Vec<(u8, HabitFrequency)> {
    habit_rows(shares.iter().map(|share| (String::new(), *share)))
        .into_iter()
        .map(|row| (row.relative, row.frequency))
        .collect()
}

#[test]
fn a_bar_is_the_habit_s_part_of_its_list_even_when_the_shares_do_not_add_up() {
    // Sign-offs as a model gave them: 62% in all.
    assert_eq!(
        rows(&[20, 22, 10, 10]),
        [
            (32, HabitFrequency::Often),
            (35, HabitFrequency::Often),
            (16, HabitFrequency::Sometimes),
            (16, HabitFrequency::Sometimes),
        ]
    );
}

#[test]
fn half_the_messages_or_more_is_mostly() {
    assert_eq!(rows(&[60, 25])[0].1, HabitFrequency::Mostly);
    assert_eq!(HabitFrequency::of(49), HabitFrequency::Often);
    assert_eq!(HabitFrequency::of(19), HabitFrequency::Sometimes);
}

#[test]
fn unknown_shares_are_drawn_in_equal_parts() {
    assert_eq!(
        rows(&[0, 0, 0]),
        [
            (33, HabitFrequency::Sometimes),
            (33, HabitFrequency::Sometimes),
            (33, HabitFrequency::Sometimes),
        ]
    );
    assert!(rows(&[]).is_empty());
}
