//! Providers populate the non-source scopes (VBA library, COM typelibs, sibling
//! /referenced projects, host) and answer member-resolution queries.

pub mod com;
pub mod declare;
pub mod host;
pub mod project;
pub mod vba_library;
