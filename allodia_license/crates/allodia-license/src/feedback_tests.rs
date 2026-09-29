use std::sync::{Arc, Mutex};

use mailcal_ai::{HttpRequest, HttpResponse, HttpTransport, Mode, TransportFailed};

use super::{AI_FEEDBACK_ROUTE_LIVE, FeedbackError, FeedbackSender};
use crate::AccountService;

/// What the transport was sent: the URL, the bearer, the body.
type Sent = Arc<Mutex<Vec<(String, Option<String>, String)>>>;

/// Answers each request with the next status, the last one repeating, and keeps what it was sent.
struct Canned {
    statuses: Mutex<Vec<u16>>,
    sent: Sent,
}

impl HttpTransport for Canned {
    fn post_json(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportFailed> {
        self.sent.lock().unwrap().push((
            request.url.to_owned(),
            request.bearer.map(str::to_owned),
            request.body.to_owned(),
        ));
        let mut statuses = self.statuses.lock().unwrap();
        let status = if statuses.len() > 1 {
            statuses.remove(0)
        } else {
            statuses[0]
        };
        if status == 0 {
            return Err(TransportFailed);
        }
        Ok(HttpResponse {
            status,
            body: "{}".to_owned(),
        })
    }
}

fn sender(statuses: &[u16], mode: Mode) -> (FeedbackSender, Sent) {
    let sent = Sent::default();
    let sender = FeedbackSender::new(
        &AccountService::new("https://mailcal.example.eu/"),
        Box::new(Canned {
            statuses: Mutex::new(statuses.to_vec()),
            sent: Arc::clone(&sent),
        }),
        Arc::new(move || mode),
    );
    (sender, sent)
}

const ITEMS: [(&str, &str); 3] = [
    ("f1", "{\"a\":1}"),
    ("f2", "{\"b\":2}"),
    ("f3", "{\"c\":3}"),
];

#[test]
fn an_item_is_posted_as_it_is_with_the_token_in_every_mode() {
    for mode in Mode::ALL {
        let (sender, sent) = sender(&[202], mode);
        assert_eq!(sender.send("access-token", "{\"a\":1}"), Ok(()));
        let sent = sent.lock().unwrap();
        assert_eq!(
            sent[0],
            (
                "https://mailcal.example.eu/api/v1/ai/feedback".to_owned(),
                Some("access-token".to_owned()),
                "{\"a\":1}".to_owned(),
            )
        );
    }
}

#[test]
fn only_a_2xx_answer_delivers() {
    for (status, expected) in [
        (200, Ok(())),
        (401, Err(FeedbackError::Status(401))),
        (404, Err(FeedbackError::Status(404))),
        (503, Err(FeedbackError::Status(503))),
        (0, Err(FeedbackError::Unreachable)),
    ] {
        let (sender, _) = sender(&[status], Mode::EuNative);
        assert_eq!(sender.send("t", "{}"), expected, "{status}");
    }
}

#[test]
fn delivery_stops_at_the_first_item_not_taken_and_keeps_the_rest() {
    let (partly, sent) = sender(&[201, 503, 201], Mode::EuNative);
    let delivery = partly.deliver_now(|| Some("t".to_owned()), ITEMS);
    assert_eq!(delivery.delivered, ["f1"]);
    assert_eq!(delivery.stopped, Some(FeedbackError::Status(503)));
    assert_eq!(sent.lock().unwrap().len(), 2);

    let (untokened, sent) = sender(&[200], Mode::EuNative);
    let delivery = untokened.deliver_now(|| None, ITEMS);
    assert!(delivery.delivered.is_empty());
    assert_eq!(delivery.stopped, Some(FeedbackError::Unauthorized));
    assert!(sent.lock().unwrap().is_empty());
}

#[test]
fn nothing_leaves_before_the_route_exists() {
    let (switched, sent) = sender(&[200], Mode::EuNative);
    let asked = std::cell::Cell::new(false);
    let delivery = switched.deliver(
        || {
            asked.set(true);
            Some("t".to_owned())
        },
        ITEMS,
    );
    if AI_FEEDBACK_ROUTE_LIVE {
        assert_eq!(delivery.delivered, ["f1", "f2", "f3"]);
    } else {
        assert_eq!(delivery, super::Delivery::default());
        assert!(!asked.get());
        assert!(sent.lock().unwrap().is_empty());
    }
}
