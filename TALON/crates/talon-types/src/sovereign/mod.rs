//! Sovereign data partitioning — compile-time boundary between sensitive
//! and public data. Mirrored from TR1M.
//!
//! `Sovereign<T>` is an opaque wrapper that:
//! - Prints `[REDACTED]` on Debug (prevents accidental logging)
//! - Does NOT implement Serialize, Deserialize, or Clone
//! - Can only be accessed through a `SovereignContext` guard that logs every access

pub mod audit;

pub use audit::{AuditSink, NullAuditSink, TracingAuditSink};

use std::fmt;
use std::ops::Deref;

// ---------------------------------------------------------------------------
// Sovereign<T> — opaque wrapper
// ---------------------------------------------------------------------------

pub struct Sovereign<T> {
    inner: T,
}

impl<T> Sovereign<T> {
    pub fn classify(value: T) -> Self {
        Self { inner: value }
    }
}

impl<T> fmt::Debug for Sovereign<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

// ---------------------------------------------------------------------------
// DeclassifiedRef<'a, T>
// ---------------------------------------------------------------------------

pub struct DeclassifiedRef<'a, T> {
    inner: &'a T,
}

impl<T: fmt::Debug> fmt::Debug for DeclassifiedRef<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> Deref for DeclassifiedRef<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.inner
    }
}

// ---------------------------------------------------------------------------
// SovereignContext — RAII guard for audited access
// ---------------------------------------------------------------------------

pub struct SovereignContext<'a> {
    caller: &'a str,
    sink: &'a dyn AuditSink,
}

impl<'a> SovereignContext<'a> {
    pub fn new(caller: &'a str, sink: &'a dyn AuditSink) -> Self {
        Self { caller, sink }
    }

    pub fn declassify_ref<'s, T>(&self, sovereign: &'s Sovereign<T>) -> DeclassifiedRef<'s, T> {
        self.sink
            .on_declassify(self.caller, std::any::type_name::<T>());
        DeclassifiedRef {
            inner: &sovereign.inner,
        }
    }

    pub fn declassify_owned<T>(&self, sovereign: Sovereign<T>) -> T {
        self.sink
            .on_declassify(self.caller, std::any::type_name::<T>());
        sovereign.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Debug, Clone, PartialEq)]
    struct SecretData {
        value: String,
    }

    #[test]
    fn classify_declassify_roundtrip() {
        let original = SecretData {
            value: "top-secret".into(),
        };
        let wrapped = Sovereign::classify(original.clone());
        let sink = NullAuditSink;
        let ctx = SovereignContext::new("test", &sink);
        let declassified = ctx.declassify_ref(&wrapped);
        assert_eq!(&*declassified, &original);
    }

    #[test]
    fn debug_redacted() {
        let wrapped = Sovereign::classify(SecretData {
            value: "secret-password-123".into(),
        });
        let debug_output = format!("{:?}", wrapped);
        assert_eq!(debug_output, "[REDACTED]");
    }

    #[test]
    fn audit_sink_called() {
        struct CountingSink(AtomicU32);
        impl AuditSink for CountingSink {
            fn on_declassify(&self, _caller: &str, _type_name: &str) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let sink = CountingSink(AtomicU32::new(0));
        let ctx = SovereignContext::new("test", &sink);
        let w1 = Sovereign::classify(42u32);
        let _ = ctx.declassify_ref(&w1);
        assert_eq!(sink.0.load(Ordering::SeqCst), 1);
        let w2 = Sovereign::classify(99u32);
        let _ = ctx.declassify_owned(w2);
        assert_eq!(sink.0.load(Ordering::SeqCst), 2);
    }
}
