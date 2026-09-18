//! Ed25519-signed hash chain for tamper-evident audit logging.
//!
//! Events are chained (each carries the hash of its predecessor),
//! signed by credd-core's key, and shipped to an append-only WORM sink.
