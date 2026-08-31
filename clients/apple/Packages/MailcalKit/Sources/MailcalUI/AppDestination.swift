// Which top-level surface the shell is showing.
//
// An enum rather than a flag per destination: with three of them, booleans admit states that
// cannot exist ("the calendar and contacts at once") and every reader has to prove they don't
// happen. This is the move the Android client made when Contacts became its third destination,
// and it lands here for the same reason.
//
// The raw value doubles as the `@SceneStorage` token, so window/scene restoration and the model
// speak one vocabulary, an unrecognised stored token simply restores nothing rather than
// resolving to some other surface.

/// The shell's top-level surfaces: the mailbox, the calendar, and contacts.
enum AppDestination: String {
    case mail
    case calendar
    case contacts
}
