//! Client bindings for the official Kubernetes v1.35.7 CRI v1 protocol.
//!
//! Regenerate with the `rubix-upstream` maintenance CLI; ordinary builds do not fetch inputs
//! or execute a protobuf compiler. These bindings do not implement a runtime.

pub mod runtime {
    // Preserve upstream protocol comments/boolean fields and tonic's nested RPC templates.
    // These four style exceptions apply only to deterministic generated source.
    #[allow(
        clippy::doc_lazy_continuation,
        clippy::doc_markdown,
        clippy::excessive_nesting,
        clippy::struct_excessive_bools
    )]
    pub mod v1 {
        include!("generated/runtime.v1.rs");
    }
}
