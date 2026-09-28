//! Strict declared asset inventory and encoded-byte checking only.
//!
//! Neither result authorizes installation or establishes decoded ELF/OCI integrity.
mod catalog;
mod inventory;
mod verify;
pub use catalog::{AssetId, CatalogEntry, Encoding, Kind, catalog};
pub use inventory::{
    DeclaredInventory, Delivery, InventoryError, InventoryRequest, Limits, Manifest, Scope, Variant,
};
pub use verify::{EncodedBlobMatch, VerificationError, VerificationSession};
