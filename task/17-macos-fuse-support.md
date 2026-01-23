# Task 17: macOS FUSE Support (macFUSE Integration)

## Overview

Add macOS platform support by integrating with macFUSE (formerly OSXFUSE). This will enable Tarbox to provide native filesystem mounting on macOS, similar to the existing Linux FUSE support.

## Background

### Current Status
- Linux FUSE support is fully implemented using the `fuser` crate
- macOS builds are currently disabled in the release workflow
- The `fuser` crate's build script panics on macOS: "Building without libfuse is only supported on Linux"
- **已确认**: `fuser` crate 在 macOS 上无法编译，必须使用条件编译

### macFUSE Overview
macFUSE is the macOS implementation of FUSE (Filesystem in Userspace):
- Website: https://osxfuse.github.io/
- Requires manual installation by users (not available via Homebrew core)
- Provides libfuse-compatible API
- Supports macOS 10.9+ (Intel) and macOS 11.0+ (Apple Silicon)

## Technical Challenges

### 1. Build-time Detection
- Need to detect macFUSE installation at build time
- macFUSE installs to `/usr/local/include/fuse` and `/usr/local/lib`
- May need custom build.rs logic

### 2. Runtime Dependencies
- Users must install macFUSE before using Tarbox mount feature
- Need graceful error handling if macFUSE is not installed
- Consider providing installation instructions in error messages

### 3. API Differences
- macFUSE API is mostly compatible with Linux libfuse
- Some minor differences in mount options and behavior
- Need to test all FUSE operations on macOS

### 4. Code Signing & Notarization
- macOS Gatekeeper may block unsigned FUSE filesystems
- May need Apple Developer account for proper distribution
- System Extension approval required on newer macOS versions

## Required: Conditional Compilation

由于 `fuser` crate 在 macOS 上无法编译，必须先实现条件编译开关才能在 macOS 上构建 Tarbox。

### 需要修改的文件

**Cargo.toml**:
```toml
[features]
default = ["fuse"]
fuse = ["dep:fuser"]

[dependencies]
fuser = { version = "0.16.0", features = ["abi-7-31"], optional = true }
```

**src/lib.rs**:
```rust
#[cfg(feature = "fuse")]
pub mod fuse;
```

**src/main.rs**:
```rust
#[cfg(feature = "fuse")]
use tarbox::fuse::{MountOptions, mount, unmount};

// Mount/Umount 命令也需要条件编译
#[cfg(feature = "fuse")]
Commands::Mount { ... } => { ... }

#[cfg(feature = "fuse")]
Commands::Umount { ... } => { ... }

#[cfg(not(feature = "fuse"))]
Commands::Mount { .. } | Commands::Umount { .. } => {
    eprintln!("FUSE support is not available on this platform.");
    eprintln!("Build with --features fuse on Linux to enable mount functionality.");
    std::process::exit(1);
}
```

**CLI 命令定义** (也需要条件编译或提供友好错误):
```rust
#[derive(Subcommand)]
enum Commands {
    // ... other commands ...
    
    #[command(about = "Mount filesystem via FUSE (Linux only)")]
    Mount { ... },
    
    #[command(about = "Unmount FUSE filesystem (Linux only)")]
    Umount { ... },
}
```

### 构建方式

```bash
# Linux (默认启用 FUSE)
cargo build --release

# macOS (禁用 FUSE)
cargo build --release --no-default-features

# 显式启用 FUSE (Linux)
cargo build --release --features fuse
```

## Implementation Plan

### Phase 0: Conditional Compilation (前置条件) ⚠️
- [ ] 将 `fuser` 改为 optional dependency
- [ ] 添加 `fuse` feature flag (默认启用)
- [ ] 给 `src/fuse/` 模块添加 `#[cfg(feature = "fuse")]`
- [ ] 给 `src/main.rs` 中的 FUSE 相关代码添加条件编译
- [ ] 更新 release workflow: macOS 使用 `--no-default-features`
- [ ] 测试 Linux 和 macOS 构建都能成功

### Phase 1: Local Development Support
- [ ] Document macFUSE installation requirements
- [ ] Update `fuser` crate configuration for macOS
- [ ] Add conditional compilation for platform-specific code
- [ ] Create macOS-specific build instructions

### Phase 2: Feature Flag Implementation
- [ ] Add `macos-fuse` feature flag to Cargo.toml (与 Linux fuse 分开)
- [ ] Gate macFUSE-specific code behind feature flag
- [ ] Ensure clean build on macOS without FUSE support
- [ ] Update lib.rs and main.rs with conditional compilation

### Phase 3: Testing & Validation
- [ ] Set up macOS test environment (Intel or Apple Silicon)
- [ ] Test basic mount/unmount operations
- [ ] Test file operations (read, write, mkdir, etc.)
- [ ] Test layer operations through mounted filesystem
- [ ] Performance testing on macOS

### Phase 4: CI/CD Integration
- [ ] Add macOS build to release workflow (with macFUSE)
- [ ] Consider separate "macos-no-fuse" build for API-only usage
- [ ] Update release notes with macOS installation instructions

### Phase 5: Documentation
- [ ] Add macOS installation guide to README
- [ ] Document macFUSE installation steps
- [ ] Add troubleshooting section for common macOS issues
- [ ] Update platform support matrix

## Dependencies

- **External**: macFUSE 4.x installed on build machine
- **Crate**: `fuser` with macOS support enabled
- **Testing**: Access to macOS development environment

## Acceptance Criteria

1. Tarbox can be built on macOS with FUSE support
2. Mount/unmount operations work correctly on macOS
3. All file operations pass tests on macOS
4. Clear error message when macFUSE is not installed
5. Documentation covers macOS setup and usage
6. Release workflow produces macOS binaries

## Estimated Effort

- **Phase 1-2**: 1-2 days (code changes)
- **Phase 3**: 2-3 days (testing, requires macOS environment)
- **Phase 4-5**: 1 day (CI/CD and docs)
- **Total**: ~1 week

## Blockers

- **No macOS test environment available** - Cannot proceed with implementation until macOS development/test environment is available
- May need Apple Developer account for code signing

## References

- macFUSE: https://osxfuse.github.io/
- fuser crate: https://docs.rs/fuser/
- macOS System Extensions: https://developer.apple.com/documentation/systemextensions
- FUSE for macOS guide: https://github.com/osxfuse/osxfuse/wiki

## Notes

This task is blocked pending availability of a macOS development environment. The implementation approach may need adjustment based on actual testing results.

Alternative approaches to consider:
1. Use NFS or WebDAV instead of FUSE for macOS
2. Provide API-only binary for macOS (no mount support)
3. Wait for fuser crate to add better macOS support
