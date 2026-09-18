//! Wire protocol types for credd-core ↔ guard communication.
//!
//! Length-prefixed frames over a Unix domain socket. No JSON —
//! the protocol is deliberately minimal to keep the core's attack surface small.
