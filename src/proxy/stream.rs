//! @agents-index The response body stream: taps each chunk, then hands the exchange record over at the end, on error, or on drop.

use crate::capture::record::ExchangeRecord;
use crate::capture::tap::Tap;
use crate::capture::writer::Capture;
use bytes::Bytes;
use futures_util::Stream;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

type Inner = Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>;

/// The record without the response body parts; the stream fills those at the end.
pub struct Pending {
    pub record: ExchangeRecord,
    pub request_tap: Arc<Mutex<Tap>>,
    pub capture: Capture,
    /// The name of the proxy token for the log line, or `none`; never the raw token or its hash.
    pub token_name: String,
}

impl Pending {
    /// Completes the record from both taps, submits it, and logs one line with no header value or body.
    pub fn finish(mut self, response: Tap, outcome: &str, error: Option<String>) {
        let r = &mut self.record;
        let req = std::mem::take(&mut *self.request_tap.lock().unwrap_or_else(|p| p.into_inner()));
        r.request_size = req.size;
        r.request_truncated = req.truncated;
        r.request_body = req.kept;
        r.response_size = response.size;
        r.response_truncated = response.truncated;
        r.response_body = response.kept;
        r.ended_ms = chrono::Utc::now().timestamp_millis();
        r.outcome = outcome.into();
        r.error = error;
        tracing::info!(
            id = %r.id, agent = %r.agent, upstream = %r.upstream, method = %r.method, path = %r.path,
            status = r.status, request_size = r.request_size, response_size = r.response_size,
            ttfb_ms = r.ttfb_ms, duration_ms = r.ended_ms - r.started_ms, outcome = %r.outcome,
            token = %self.token_name, "exchange"
        );
        self.capture.submit(self.record);
    }
}

/// Passes each upstream chunk through unchanged; the copy into the tap never fails the stream.
pub struct CaptureStream {
    inner: Inner,
    tap: Tap,
    /// The body length the client expects, if known; the server stops polling once it is sent.
    expected: Option<u64>,
    pending: Option<Pending>,
}

impl CaptureStream {
    pub fn new(inner: Inner, limit: usize, expected: Option<u64>, pending: Pending) -> Self {
        CaptureStream {
            inner,
            tap: Tap::new(limit),
            expected,
            pending: Some(pending),
        }
    }

    fn all_sent(&self) -> bool {
        self.expected.is_some_and(|n| self.tap.size >= n)
    }

    fn end(&mut self, outcome: &str, error: Option<String>) {
        if let Some(p) = self.pending.take() {
            p.finish(std::mem::take(&mut self.tap), outcome, error);
        }
    }
}

impl Stream for CaptureStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(b))) => {
                self.tap.push(&b);
                if self.all_sent() {
                    self.end("complete", None);
                }
                Poll::Ready(Some(Ok(b)))
            }
            Poll::Ready(Some(Err(e))) => {
                let cause = format!(
                    "the upstream closed during the response: {}",
                    e.without_url()
                );
                self.end("upstream_aborted", Some(cause.clone()));
                Poll::Ready(Some(Err(std::io::Error::other(cause))))
            }
            Poll::Ready(None) => {
                self.end("complete", None);
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for CaptureStream {
    /// A drop before the end means the client went away; dropping `inner` closes the upstream request.
    fn drop(&mut self) {
        if self.all_sent() {
            self.end("complete", None);
        }
        self.end(
            "client_aborted",
            Some("the client closed the connection during the response".into()),
        );
    }
}
