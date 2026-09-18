//! Egress scanning: secret fingerprints, vendor key pattern matching,
//! and high-entropy string detection with normalization.
//!
//! Normalizes base64, hex, percent-encoding, JSON escapes, and
//! whitespace-split forms before matching. Blocks rather than redacts.
