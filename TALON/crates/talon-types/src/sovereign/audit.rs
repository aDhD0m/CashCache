//! Audit sinks for sovereign data declassification events.

pub trait AuditSink: Send + Sync {
    fn on_declassify(&self, caller: &str, type_name: &str);
}

pub struct TracingAuditSink;

impl AuditSink for TracingAuditSink {
    fn on_declassify(&self, caller: &str, type_name: &str) {
        tracing::trace!(
            caller = caller,
            sovereign_type = type_name,
            "sovereign data declassified"
        );
    }
}

pub struct NullAuditSink;

impl AuditSink for NullAuditSink {
    fn on_declassify(&self, _caller: &str, _type_name: &str) {}
}
