// SPDX-License-Identifier: MPL-2.0
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

//! ## Overview
//!
//! Safe, idiomatic Rust bindings for [Prism](https://github.com/ethindp/prism),
//! the Platform-agnostic Reader Interface for Speech and Messages.
//!
//! The entry point is [`Context`]. From a context you enumerate and create
//! [`Backend`]s. You can also register your own pure-Rust backends via
//! [`RegistryBuilder`]/[`CustomBackend`] and run a context against them — which
//! is exactly how this crate's own tests exercise the full API without any real
//! screen reader present.
//!
//! ```no_run
//! use prism::Context;
//!
//! # fn main() -> prism::Result<()> {
//! let ctx = Context::new()?;
//! let mut backend = ctx.acquire_best()?;
//! backend.speak("Hello from Rust", false)?;
//! # Ok(())
//! # }
//! ```

mod backend;
mod context;
mod error;
pub mod log;
mod registry;
mod util;
mod version;

pub use backend::Backend;
pub use context::{Context, ContextBuilder};
pub use error::{error_string, Error, Result};
pub use registry::{CustomBackend, Registry, RegistryBuilder};
pub use version::{runtime_version, runtime_version_string};

/// Value types shared with the raw FFI layer (error codes, backend ids,
/// capability flags, log levels).
pub use prism_types::{BackendFeatures, BackendId, LogLevel, CONFIG_VERSION, PLUGIN_ABI_VERSION};

/// Access to the raw `-sys` FFI layer for advanced/unsupported use cases.
pub use prism_sys as sys;
