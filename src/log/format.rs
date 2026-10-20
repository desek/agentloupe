//! @agents-index Formats one tracing event as one JSON line with UTC time, level, file:line, message, and key fields.

use std::fmt;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

/// The event formatter for every log channel; one event is one line.
pub struct JsonLine;

/// Collects the event fields; `message` is kept apart from the key fields.
#[derive(Default)]
struct Fields {
    message: String,
    rest: serde_json::Map<String, serde_json::Value>,
}

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let text = format!("{value:?}");
        if field.name() == "message" {
            self.message = text;
        } else {
            self.rest.insert(field.name().into(), text.into());
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.into();
        } else {
            self.rest.insert(field.name().into(), value.into());
        }
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.rest.insert(field.name().into(), value.into());
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.rest.insert(field.name().into(), value.into());
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.rest.insert(field.name().into(), value.into());
    }
}

impl<S, N> FormatEvent<S, N> for JsonLine
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut w: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let meta = event.metadata();
        let mut fields = Fields::default();
        event.record(&mut fields);
        let line = serde_json::json!({
            "time": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "level": meta.level().as_str(),
            "file": format!("{}:{}", meta.file().unwrap_or("unknown"), meta.line().unwrap_or(0)),
            "message": fields.message,
            "fields": fields.rest,
        });
        writeln!(w, "{line}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::layer::SubscriberExt;

    #[derive(Clone, Default)]
    struct Buf(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Buf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn line_has_source_location() {
        let buf = Buf::default();
        let out = buf.clone();
        let sub = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .event_format(JsonLine)
                .with_writer(move || out.clone()),
        );
        tracing::subscriber::with_default(sub, || tracing::info!(port = 1u64, "started"));
        let text = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap();
        assert_eq!(text.lines().count(), 1);
        let v: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let file = v["file"].as_str().unwrap();
        let (path, line) = file.rsplit_once(':').unwrap();
        assert!(
            path.ends_with(".rs") && line.parse::<u32>().is_ok(),
            "{file}"
        );
        assert_eq!(v["message"], "started");
        assert_eq!(v["level"], "INFO");
        assert_eq!(v["fields"]["port"], 1);
    }
}
