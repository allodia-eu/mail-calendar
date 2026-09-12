//! The vocabulary the British-English rule is decided against.
//!
//! Separated from the checker so the rule's code and the rule's word lists can each be read without
//! the other, and so adding a spelling is an edit to a list rather than to a search.

/// American spelling to the British one it should be.
///
/// Longest first within a family, so the entry a reader scanning for `organization` finds is the
/// one that matches it rather than the shorter stem underneath.
pub(crate) const SPELLINGS: &[(&str, &str)] = &[
    ("behavioral", "behavioural"),
    ("behaviors", "behaviours"),
    ("behavior", "behaviour"),
    ("colors", "colours"),
    ("colored", "coloured"),
    ("coloring", "colouring"),
    ("color", "colour"),
    ("organizations", "organisations"),
    ("organization", "organisation"),
    ("organizers", "organisers"),
    ("organizer", "organiser"),
    ("organized", "organised"),
    ("organizing", "organising"),
    ("organizes", "organises"),
    ("organize", "organise"),
    ("sanitizations", "sanitisations"),
    ("sanitization", "sanitisation"),
    ("sanitizers", "sanitisers"),
    ("sanitizer", "sanitiser"),
    ("sanitized", "sanitised"),
    ("sanitizing", "sanitising"),
    ("sanitizes", "sanitises"),
    ("sanitize", "sanitise"),
    ("normalization", "normalisation"),
    ("normalized", "normalised"),
    ("normalizing", "normalising"),
    ("normalizes", "normalises"),
    ("normalize", "normalise"),
    ("centered", "centred"),
    ("centering", "centring"),
    ("centers", "centres"),
    ("center", "centre"),
    ("canceled", "cancelled"),
    ("canceling", "cancelling"),
    ("analyzed", "analysed"),
    ("analyzing", "analysing"),
    ("analyze", "analyse"),
    ("recognized", "recognised"),
    ("recognizing", "recognising"),
    ("recognizes", "recognises"),
    ("recognize", "recognise"),
    ("authorization", "authorisation"),
    ("authorized", "authorised"),
    ("authorizing", "authorising"),
    ("authorizes", "authorises"),
    ("authorize", "authorise"),
    ("customization", "customisation"),
    ("customized", "customised"),
    ("customizes", "customises"),
    ("customize", "customise"),
    ("optimization", "optimisation"),
    ("optimized", "optimised"),
    ("optimizing", "optimising"),
    ("optimizes", "optimises"),
    ("optimize", "optimise"),
    ("initialization", "initialisation"),
    ("initialized", "initialised"),
    ("initializing", "initialising"),
    ("initializes", "initialises"),
    ("initialize", "initialise"),
    ("localization", "localisation"),
    ("localized", "localised"),
    ("localizing", "localising"),
    ("localizes", "localises"),
    ("localize", "localise"),
    ("summarized", "summarised"),
    ("summarizing", "summarising"),
    ("summarizes", "summarises"),
    ("summarize", "summarise"),
    ("utilization", "utilisation"),
    ("utilized", "utilised"),
    ("utilizing", "utilising"),
    ("utilizes", "utilises"),
    ("utilize", "utilise"),
    ("synchronization", "synchronisation"),
    ("synchronized", "synchronised"),
    ("synchronizing", "synchronising"),
    ("synchronizes", "synchronises"),
    ("synchronize", "synchronise"),
    ("visualization", "visualisation"),
    ("visualized", "visualised"),
    ("visualizing", "visualising"),
    ("visualize", "visualise"),
    ("realized", "realised"),
    ("realizing", "realising"),
    ("realizes", "realises"),
    ("realize", "realise"),
    ("minimized", "minimised"),
    ("minimizing", "minimising"),
    ("minimizes", "minimises"),
    ("minimize", "minimise"),
    ("maximized", "maximised"),
    ("maximizing", "maximising"),
    ("maximizes", "maximises"),
    ("maximize", "maximise"),
    ("categorized", "categorised"),
    ("categorizes", "categorises"),
    ("categorize", "categorise"),
    ("prioritized", "prioritised"),
    ("prioritizes", "prioritises"),
    ("prioritize", "prioritise"),
    ("generalized", "generalised"),
    ("generalizes", "generalises"),
    ("generalize", "generalise"),
    ("apologize", "apologise"),
    ("labeled", "labelled"),
    ("labeling", "labelling"),
    ("modeling", "modelling"),
    ("traveling", "travelling"),
    ("honored", "honoured"),
    ("honoring", "honouring"),
    ("honors", "honours"),
    ("honor", "honour"),
    ("favored", "favoured"),
    ("favoring", "favouring"),
    ("favors", "favours"),
    ("favor", "favour"),
    ("defense", "defence"),
    ("offense", "offence"),
];

/// Vocabulary somebody else chose, compared with case and hyphens flattened.
///
/// A phrase is multi-word on purpose: no phrase can swallow an ordinary sentence the way a
/// lowercased bare word would, which is what makes the flattening safe here.
pub(crate) const PHRASES: &[&str] = &[
    // OAuth, OpenID and the RFCs that name them.
    "authorization server",
    "authorization endpoint",
    "authorization code",
    "authorization request",
    "authorization grant",
    "authorization response",
    "authorization url",
    "authorization flow",
    "authorization header",
    "authorization_endpoint",
    "authorization_code",
    "authorization_url",
    "rfc 6749",
    "rfc 8414",
    "rfc 7591",
    "rfc 9728",
    // The console a Windows release is submitted to.
    "partner center",
];

/// A single token, compared exactly.
///
/// `Color` is a Swift type and `color` is an ordinary word: flattening the case would make the type
/// name excuse every use of the word.
pub(crate) const SYMBOLS: &[&str] = &[
    // iCalendar, serde and REUSE.
    "ORGANIZER",
    "invitation_organizer",
    "organizer_line",
    "serialize",
    "deserialize",
    "Serialize",
    "Deserialize",
    "SPDX-License-Identifier",
    "LicenseRef",
    "LICENSES",
    "LICENSE",
    // Toolkit, language and platform keywords, and the tools a build actually invokes.
    "authorizationUrl",
    "Authorization:",
    "@Synchronized",
    "synchronized(",
    "Synchronized",
    "isMaximized",
    "WindowState",
    "NSLocalizedString",
    "LocalizedStringKey",
    "localizedDescription",
    "LocalizedError",
    "initializer",
    "Initializer",
    "recognizer",
    "Recognizer",
    "colorScheme",
    "color:",
    "Color",
    "Colors",
    "colorResource",
    "grayscale",
    "upload-artifact",
    "download-artifact",
    "normalize_platform",
    "summarize_repeat",
    "AppCulture",
    // A doc reference names a symbol from inside a comment.
    "cref=",
    "paramref name=",
    "typeparamref name=",
];

/// Not ours to rewrite, or not rewritten for a reason stated in AGENTS.md.
pub(crate) const EXEMPT: &[&str] = &[
    "CODE_OF_CONDUCT.md",       // Contributor Covenant, CC-BY-4.0, upstream text
    "LICENSES/",                // licence texts, upstream
    "clients/android/gradle",   // the Gradle wrapper, upstream
    "clients/composer/dist/",   // generated: the committed editor bundle
    "docs/changelog/released/", // history: a released note is what shipped
    "docs/changelog/announcements/", // the same
    "docs/privacy-policy",      // a published contract; its text moves with a version bump
    "xtask/src/british_english.rs", // the rule,
    "xtask/src/british_english_words.rs", // and the vocabulary it is decided against
];

/// The file types the rule reaches.
pub(crate) const EXTENSIONS: &[&str] = &[
    "md", "rs", "swift", "kt", "kts", "cs", "ts", "js", "py", "sh", "ps1", "toml", "yml", "yaml",
    "xml", "html",
];
