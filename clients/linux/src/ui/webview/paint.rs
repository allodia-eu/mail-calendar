//! Whether the reading view's document has painted yet, which decides when the view is revealed.

/// Whether the document last handed to [`super::SecureWebView::load`] has finished painting.
///
/// The page a message renders is white, but the surface WebKit composites it on is not: on a
/// large view the web process presents a **black** frame until the document's first paint, and on
/// a heavy message that is hundreds of milliseconds (467 ms, measured, at full screen). A host
/// that reveals the view as soon as it asks for a document therefore shows black in the middle of
/// the reading pane's page. `webkit_web_view_set_background_color` does not reach it: that colour
/// is what the *page* renders on, not what the compositor presents before there is a page.
///
/// `Finished` alone does not answer the question, which is why this is a pair rather than a flag.
/// A load cancelled by the next `load_html` still reaches `Finished`, and it arrives **after** the
/// call that cancelled it, so the naive reading hands the previous message's completion to the
/// message now loading and reveals an unpainted view. Requiring a `Started` since the last ask
/// separates the two. The measured sequences are the fixtures `webview_tests.rs` drives.
#[derive(Default)]
pub(super) struct PaintGate {
    /// Whether a load has begun since the last [`Self::asked`]. Only a `Finished` after one
    /// belongs to the document we are waiting for.
    started: bool,
    painted: bool,
}

impl PaintGate {
    /// A document was handed to the view: nothing on screen belongs to it yet.
    pub(super) fn asked(&mut self) {
        self.started = false;
        self.painted = false;
    }

    /// Folds in one `load-changed`, and reports whether the page just arrived.
    pub(super) fn observed(&mut self, event: webkit6::LoadEvent) -> bool {
        match event {
            webkit6::LoadEvent::Started => {
                self.started = true;
                false
            }
            webkit6::LoadEvent::Finished if self.started => {
                self.painted = true;
                true
            }
            _ => false,
        }
    }

    pub(super) fn painted(&self) -> bool {
        self.painted
    }
}
