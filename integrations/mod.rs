//! External integration modules for Omo-Koda2.
//!
//! Each sub-module provides standalone async HTTP functions for one external
//! service.  The Tool wrappers in `crate::tools` call these functions — this
//! is the single source of truth for wire protocol details.
pub mod ucx;
