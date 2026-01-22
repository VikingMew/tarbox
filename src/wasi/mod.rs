// WASI module - WebAssembly System Interface support
//
// This module provides WASI filesystem interface implementation for Tarbox,
// allowing it to run in WebAssembly runtimes (Wasmtime, Spin, WasmEdge, browsers).

pub mod adapter;
pub mod config;
pub mod error;
pub mod fd_table;

pub use adapter::WasiAdapter;
pub use config::{DbMode, WasiConfig};
pub use error::{WasiError, to_wasi_errno};
pub use fd_table::{FdTable, FileDescriptor, OpenFlags};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all public types are accessible
        let _: Option<WasiConfig> = None;
        let _: Option<WasiError> = None;
        let _: Option<FdTable> = None;
    }
}
