// build.rs — Python SDK build script.
//
// This file exists for ONE reason: pyo3's `extension-module` feature.
//
// When `extension-module` is on, pyo3 deliberately does NOT link libpython
// at build time. Python provides the required C API symbols at runtime when
// it loads the .so / .dylib via `import`. This is correct for production
// extension modules — it lets a single wheel work across Python ABIs that
// expose the same stable ABI surface (CPython 3.9+ for our `abi3-py39`).
//
// The catch: on macOS, the linker (`ld64`) refuses by default to leave
// undefined symbols in a cdylib. Without an explicit instruction to defer
// resolution to runtime, `cargo build --workspace` on `aarch64-apple-darwin`
// (and `x86_64-apple-darwin`) fails with:
//
//     ld: Undefined symbols for architecture arm64:
//       "_PyObject_GetAttrString", referenced from: ...
//       "_PyTuple_New", ...
//     clang: error: linker command failed with exit code 1
//
// maturin sets `-C link-arg=-undefined -C link-arg=dynamic_lookup` itself
// when building wheels (it knows about pyo3's `extension-module` mode). But
// `cargo build` outside maturin — which is what our `multi-arch-release.yml`
// does in step "Build Rust workspace (release)" — has no such handling.
// Hence this build.rs: it emits the same flags ONLY on macOS, where they
// are needed and not harmful.
//
// On Linux: the linker is happy with undefined symbols in a cdylib by
// default, so no flags are needed.
// On Windows: pyo3 still links pythonXY.lib statically when building with
// `extension-module` (Windows doesn't have macOS's deferred-symbol
// equivalent), so no flags are needed.
//
// References:
//   * pyo3 / maturin: https://pyo3.rs/v0.28/building-and-distribution
//   * macOS ld64 -undefined: man ld(1)

fn main() {
    // CARGO_CFG_TARGET_OS is set by cargo for every build script. Compare
    // against the value `cargo` itself uses for macOS.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        // The flag is `-undefined dynamic_lookup`, but rustc passes link
        // args through to the linker as-is — we have to emit them as two
        // separate `link-arg` directives.
        println!("cargo:rustc-link-arg=-undefined");
        println!("cargo:rustc-link-arg=dynamic_lookup");
    }
}
