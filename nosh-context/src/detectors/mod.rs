//! Project context detectors.
//!
//! Each detector handles a specific type of project file or tool.

pub mod bun;
pub mod cpp;
pub mod docker;
pub mod git;
pub mod go;
pub mod node;
pub mod package;
pub mod python;
pub mod rust;
