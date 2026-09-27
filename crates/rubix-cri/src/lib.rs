//! Client bindings for the official Kubernetes v1.35.7 CRI v1 protocol.
//!
//! Regenerate with `tools/upstream/upstream.py`; ordinary builds do not fetch inputs
//! or execute a protobuf compiler. These bindings do not implement a runtime.

// Generated protocol code has upstream names and mechanical conversion patterns.
// Keep repository lint policy on the handwritten wrapper, not generator output.
#[allow(clippy::all, clippy::pedantic, missing_debug_implementations)]
pub mod runtime {
    pub mod v1 {
        include!("generated/runtime.v1.rs");
    }
}
