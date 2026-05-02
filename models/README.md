# Mneme bundled models - runtime requirements

This directory houses the model files that `mneme models install --from-path`
copies into `~/.mneme/models/`. The bundle is **flat** - every recognised
file lives at the top level; nested subdirectories are not walked.

## Recognised file types

| Pattern | Detected kind | Use |
|---|---|---|
| `*.onnx`                   | `embedding-model`     | BGE-Small-En-v1.5 (384-dim sentence encoder) |
| `tokenizer.json`           | `embedding-tokenizer` | HuggingFace tokenizer paired with BGE |
| `*.gguf` / `*.ggml` / `*.bin` containing `embed` (case-insensitive) | `embedding-llm` | Embed LLMs (e.g. `qwen-embed-0.5b.gguf`, `nomic-embed-text.gguf`) |
| `*.gguf` / `*.ggml` / `*.bin` (otherwise)                            | `llm`           | Generative LLMs (e.g. `phi-3-mini-4k.gguf`, `qwen-coder-0.5b.gguf`) |

Anything else (READMEs, configs, `.installed` markers) is silently
skipped at registration time but listed in stderr so the user sees
what was passed over.

After install, mneme writes `~/.mneme/models/manifest.json` with one
entry per registered file:

```json
{
  "version": 1,
  "entries": [
    {
      "name": "bge-small-en-v1.5.onnx",
      "kind": "embedding-model",
      "size": 132956160,
      "path": "bge-small-en-v1.5.onnx"
    }
  ]
}
```

`mneme doctor` reads this manifest and renders the per-kind health box.

## ONNX Runtime - required for the BGE embedder

The BGE-Small-En-v1.5 ONNX model needs the **ONNX Runtime** native
shared library at runtime:

| OS      | Library                  | Where mneme looks                                     |
|---------|--------------------------|--------------------------------------------------------|
| Windows | `onnxruntime.dll`        | `~/.mneme/bin/onnxruntime.dll` (auto-pinned via `ORT_DYLIB_PATH` on first BGE call); falls back to `PATH` |
| Linux   | `libonnxruntime.so`      | `~/.mneme/bin/libonnxruntime.so` (auto-pinned); falls back to `LD_LIBRARY_PATH` |
| macOS   | `libonnxruntime.dylib`   | `~/.mneme/bin/libonnxruntime.dylib` (auto-pinned); falls back to `DYLD_LIBRARY_PATH` |

**As of v0.3.2 mneme DOES bundle this DLL** in `~/.mneme/bin/`.
ONNX Runtime 1.24.4 (matches the `ort 2.0.0-rc.12` API-24 ABI) is
included in the release zip. On first BGE call the `brain` crate
auto-pins `ORT_DYLIB_PATH` to the bundled file so the in-tree version
always wins over any stale Win11 24H2 System32 copy. The manual
install steps below are kept for source builds and air-gapped users
who don't run the bootstrap.

### Manual install (Windows)

```powershell
# 1. Download the latest ONNX Runtime release (CPU build):
Invoke-WebRequest `
  -Uri "https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/onnxruntime-win-x64-1.20.1.zip" `
  -OutFile "$env:TEMP\onnxruntime.zip"

# 2. Extract and copy the DLL into ~/.mneme/bin (which install.ps1 adds to PATH):
Expand-Archive -Path "$env:TEMP\onnxruntime.zip" -DestinationPath "$env:TEMP\ort"
Copy-Item "$env:TEMP\ort\onnxruntime-win-x64-1.20.1\lib\onnxruntime.dll" `
          -Destination "$env:USERPROFILE\.mneme\bin\"

# 3. Verify
mneme doctor   # should show the embedding-model row in the local-models box
```

### Manual install (Linux)

```bash
curl -L https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/onnxruntime-linux-x64-1.20.1.tgz \
  | tar -xz -C /tmp
cp /tmp/onnxruntime-linux-x64-1.20.1/lib/libonnxruntime.so.1.20.1 ~/.mneme/bin/
ln -sf libonnxruntime.so.1.20.1 ~/.mneme/bin/libonnxruntime.so
```

### Manual install (macOS)

```bash
curl -L https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/onnxruntime-osx-arm64-1.20.1.tgz \
  | tar -xz -C /tmp
cp /tmp/onnxruntime-osx-arm64-1.20.1/lib/libonnxruntime.1.20.1.dylib ~/.mneme/bin/
ln -sf libonnxruntime.1.20.1.dylib ~/.mneme/bin/libonnxruntime.dylib
```

### Auto-install (preferred, v0.3.3+)

```bash
mneme models install-onnx-runtime
```

In v0.3.2 this is a stub that prints the manual procedure above. v0.3.3
will auto-fetch the official release archive, sha256-verify it, and
extract the shared library into `~/.mneme/bin/`.

## Build-time requirement: `--features real-embeddings`

The BGE embedder is feature-gated in the `brain` crate so a fresh
checkout compiles without the C++ toolchain. **As of v0.3.2 the
shipped binaries always have `real-embeddings` on**, and the bootstrap
installer auto-stages the ONNX Runtime 1.24.4 DLL alongside the model
files - so end users do nothing.

For source builds:

```bash
# Default workspace build (since v0.3.2): real-embeddings on by default
cargo build -p mneme-cli --release

# Force the pure-Rust hashing-trick fallback at runtime (no rebuild
# needed - flip this env var and recall keeps working):
$env:MNEME_FORCE_HASH_EMBED = "1"
mneme build .

# Drop the real-embeddings feature entirely (rare; use only when ORT
# system dep is unavailable):
cargo build -p mneme-cli --release --no-default-features
```

## What gets registered

The default `final.zip` bundle ships these five files at this README's
sibling level:

| File                          | Size    | Detected kind          |
|-------------------------------|---------|------------------------|
| `bge-small-en-v1.5.onnx`      | ~130 MB | `embedding-model`      |
| `tokenizer.json`              | ~1 KB   | `embedding-tokenizer`  |
| `phi-3-mini-4k.gguf`          | ~2.28 GB| `llm`                  |
| `qwen-coder-0.5b.gguf`        | ~469 MB | `llm`                  |
| `qwen-embed-0.5b.gguf`        | ~609 MB | `embedding-llm`        |

Run

```bash
mneme models install --from-path <path-to-this-models-dir>
mneme doctor
```

and `mneme doctor` will render all five rows under `local models`.

## Verifying the install

```bash
mneme models status   # shows the BGE row + bundle manifest summary
mneme doctor          # full per-kind box with totals
```

If `mneme doctor` shows the embedding-model row but BGE recall is still
falling back to the hashing-trick embedder, the most likely cause is a
missing `onnxruntime.dll` (Windows). Run `mneme models
install-onnx-runtime` for the fix recipe.
