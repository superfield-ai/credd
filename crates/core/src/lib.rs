//! credd-core: sealed secret store, envelope AEAD, KEK management, audit signing,
//! and multisig verification.
//!
//! This is the privileged process. It owns the unsealed key material and has
//! no network listener and no JSON parser for untrusted input.
