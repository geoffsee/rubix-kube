//! Strict declared asset inventory, encoded/decoded-byte checks and bounded identity ELF observations.
//!
//! These results do not authorize installation or prove runtime/ABI compatibility or OCI integrity.
mod catalog;
mod decode;
pub use decode::{DecodeError, DecodeLimits, DecodePolicyError, DecodeSession, DecodedObservation};
mod elf;
pub use elf::{ArmFloatAbi, ElfError, ElfInspection, ElfLimits, LoaderFamily, LoaderRelation};
mod inventory;
mod verify;
pub use catalog::{AssetId, CatalogEntry, Encoding, Kind, catalog};
pub use inventory::{
    DeclaredInventory, Delivery, InventoryError, InventoryRequest, Limits, Manifest, Scope, Variant,
};
pub use verify::{EncodedBlobMatch, VerificationError, VerificationSession};
