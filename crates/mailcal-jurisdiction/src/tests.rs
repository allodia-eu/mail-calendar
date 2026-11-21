use super::{Class, Destination, Limit, Mode, Refused, classify, gate};

/// The whole table, written out rather than derived, so a change to the rule has to change a
/// line a reviewer reads.
#[test]
fn each_mode_admits_exactly_its_classes() {
    let admitted = |mode| {
        Class::ALL
            .into_iter()
            .filter(|class| gate(mode, *class).is_ok())
            .collect::<Vec<_>>()
    };
    assert_eq!(admitted(Mode::All), Class::ALL.to_vec());
    assert_eq!(
        admitted(Mode::EuHosted),
        vec![Class::EuNative, Class::EuHosted]
    );
    assert_eq!(admitted(Mode::EuNative), vec![Class::EuNative]);
}

#[test]
fn an_unclassified_destination_passes_only_when_everything_does() {
    for mode in Mode::ALL {
        assert_eq!(gate(mode, Class::Unknown).is_ok(), mode == Mode::All);
    }
}

#[test]
fn a_refusal_names_the_mode_and_the_class_it_did_not_admit() {
    let refused = gate(Mode::EuNative, Class::NonEu).unwrap_err();
    assert_eq!(
        refused,
        Refused::NonEu {
            mode: Limit::EuNative
        }
    );
    assert_eq!(
        refused.to_string(),
        "a non-eu destination is not admitted under eu-native"
    );
}

/// Each refusal the gate gives reads back as the mode and the class it was asked with, and each
/// of the five a refusal can express is given by some pair.
#[test]
fn a_refusal_expresses_exactly_what_the_gate_refuses() {
    let mut given = Vec::new();
    for mode in Mode::ALL {
        for class in Class::ALL {
            if let Err(refused) = gate(mode, class) {
                assert_eq!((refused.mode(), refused.class()), (mode, class));
                given.push(refused);
            }
        }
    }
    let expressible = [
        Refused::EuHosted,
        Refused::NonEu {
            mode: Limit::EuHosted,
        },
        Refused::NonEu {
            mode: Limit::EuNative,
        },
        Refused::Unknown {
            mode: Limit::EuHosted,
        },
        Refused::Unknown {
            mode: Limit::EuNative,
        },
    ];
    assert_eq!(given.len(), expressible.len());
    for refused in expressible {
        assert!(given.contains(&refused), "{refused:?} is never given");
    }
}

#[test]
fn the_relay_is_eu_native_and_an_own_endpoint_is_what_it_was_declared() {
    assert_eq!(classify(&Destination::AllodiaRelay), Class::EuNative);
    for class in Class::ALL {
        assert_eq!(
            classify(&Destination::OwnEndpoint {
                declared: Some(class)
            }),
            class
        );
    }
    assert_eq!(
        classify(&Destination::OwnEndpoint { declared: None }),
        Class::Unknown
    );
}

#[test]
fn the_default_mode_is_the_strictest() {
    assert_eq!(Mode::default(), Mode::EuNative);
}

/// The mode and a declaration serialise under their log labels, so a stored value and a log line
/// name a mode the same way.
#[test]
fn both_round_trip_through_toml_under_their_labels() {
    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct Stored {
        mode: Mode,
        class: Class,
    }
    for mode in Mode::ALL {
        for class in Class::ALL {
            let stored = Stored { mode, class };
            let body = toml::to_string(&stored).unwrap();
            assert!(body.contains(&format!("mode = \"{}\"", mode.label())));
            assert!(body.contains(&format!("class = \"{}\"", class.label())));
            assert_eq!(toml::from_str::<Stored>(&body).unwrap(), stored);
        }
    }
}
