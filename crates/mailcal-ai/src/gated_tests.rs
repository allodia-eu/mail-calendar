use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use mailcal_jurisdiction::{Class, Destination, Mode, classify, gate};

use crate::{
    AiError, GatedBackend,
    test_support::{RecordingBackend, text_answer},
    wire::{ChatMessage, ChatRequest, Purpose},
};

fn request() -> ChatRequest {
    ChatRequest {
        purpose: Purpose::Draft,
        messages: vec![ChatMessage::user("hello")],
        tools: Vec::new(),
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    }
}

fn destinations() -> Vec<Destination> {
    let mut all = vec![
        Destination::AllodiaRelay,
        Destination::OwnEndpoint { declared: None },
    ];
    all.extend(Class::ALL.map(|class| Destination::OwnEndpoint {
        declared: Some(class),
    }));
    all
}

/// Every mode against every destination: a request the gate refuses never reaches the backend,
/// and one it admits reaches it exactly once.
#[test]
fn nothing_reaches_the_backend_unless_the_gate_admits_it() {
    for mode in Mode::ALL {
        for destination in destinations() {
            let backend = RecordingBackend::answering(vec![Ok(text_answer("hi"))]);
            let seen = Arc::clone(&backend.seen);
            let gated = GatedBackend::new(Box::new(backend), destination, Arc::new(move || mode));

            let result = gated.chat(&request());

            let admitted = gate(mode, classify(&destination)).is_ok();
            assert_eq!(
                seen.lock().unwrap().len(),
                usize::from(admitted),
                "{mode:?} → {destination:?}"
            );
            match result {
                Ok(_) => assert!(admitted),
                Err(AiError::Refused(refused)) => {
                    assert!(!admitted);
                    assert_eq!(refused.mode, mode);
                    assert_eq!(refused.class, classify(&destination));
                }
                Err(other) => panic!("unexpected {other:?}"),
            }
        }
    }
}

/// The mode is read per request, so a stricter preference applies to the very next one.
#[test]
fn the_mode_is_read_at_the_moment_of_each_request() {
    let mode = Arc::new(std::sync::Mutex::new(Mode::All));
    let reading = Arc::clone(&mode);
    let backend = RecordingBackend::answering(vec![Ok(text_answer("hi"))]);
    let seen = Arc::clone(&backend.seen);
    let gated = GatedBackend::new(
        Box::new(backend),
        Destination::OwnEndpoint { declared: None },
        Arc::new(move || *reading.lock().unwrap()),
    );

    assert!(gated.chat(&request()).is_ok());
    *mode.lock().unwrap() = Mode::EuNative;
    assert!(matches!(gated.chat(&request()), Err(AiError::Refused(_))));
    assert!(gated.check().is_err());
    assert_eq!(seen.lock().unwrap().len(), 1);
}

/// Pins the shape the gate depends on: no public type of this crate implements `AiBackend`, so
/// the only way an app gets something that dispatches is through `GatedBackend`, and every
/// function that dispatches takes that type.
#[test]
fn no_public_type_of_this_crate_implements_the_backend_port() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let sources: Vec<(PathBuf, String)> = rust_files(&src)
        .into_iter()
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            !name.ends_with("tests.rs") && name != "test_support.rs"
        })
        .map(|path| {
            let text = std::fs::read_to_string(&path).unwrap();
            (path, text)
        })
        .collect();

    let mut implementors = Vec::new();
    for (_, text) in &sources {
        for line in text.lines() {
            if let Some((_, rest)) = line.trim_start().split_once("AiBackend for ")
                && line.trim_start().starts_with("impl")
            {
                implementors.push(rest.trim_end_matches(" {").trim().to_owned());
            }
        }
    }
    assert_eq!(implementors, ["OpenAiCompatibleBackend"]);
    for name in &implementors {
        let public = format!("pub struct {name}");
        assert!(
            sources.iter().all(|(_, text)| !text.contains(&public)),
            "{name} implements AiBackend and is public"
        );
    }
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    files
}
