//! Mneme Scanners library.
//!
//! Public surface re-exported here so the binary (`main.rs`) and external
//! integration tests use a stable, documented API.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod error;
pub mod job;
pub mod registry;
pub mod scanner;
pub mod scanners;
pub mod store_ipc;
pub mod worker;

#[cfg(test)]
pub mod tests;

pub use error::{Result, ScannerError};
pub use job::{ScanJob, ScanResult};
pub use registry::{RegistryConfig, ScannerRegistry};
pub use scanner::{line_col_of, Ast, Finding, Scanner, Severity};
pub use store_ipc::{BatcherConfig, FindingsBatch, StoreIpcBatcher};
pub use worker::ScanWorker;
