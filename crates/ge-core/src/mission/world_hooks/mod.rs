//! Ordered world-hook components. Real retained state and clock inputs belong
//! to their host; component completion does not publish loader/user readiness.
pub mod buffer;
pub mod clock_reset;
pub mod final_reset;
