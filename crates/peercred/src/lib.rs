//! Unix socket peer credential extraction and PID pinning.
//!
//! Uses `SO_PEERCRED` (Linux) / `LOCAL_PEERCRED` (macOS) at accept() time,
//! then pins the PID via `pidfd_open` (or `SO_PEERPIDFD` on Linux 6.5+)
//! to prevent PID recycling races.
