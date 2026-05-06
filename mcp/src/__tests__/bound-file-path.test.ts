// mcp/src/__tests__/bound-file-path.test.ts
//
// Coverage for `boundFilePath` — the M-4 audit fix that filters
// suspect file paths out of MCP tool responses before the AI sees
// them.

import { describe, expect, it } from "bun:test";
import { boundFilePath } from "../store.ts";

describe("boundFilePath", () => {
  it("passes through ordinary relative paths", () => {
    expect(boundFilePath("src/lib.rs")).toBe("src/lib.rs");
    expect(boundFilePath("vision/src/App.tsx")).toBe("vision/src/App.tsx");
    expect(boundFilePath("a.rs")).toBe("a.rs");
    expect(boundFilePath("deeply/nested/path/file.py")).toBe(
      "deeply/nested/path/file.py",
    );
  });

  it("passes through Windows-backslash relative paths", () => {
    expect(boundFilePath("src\\lib.rs")).toBe("src\\lib.rs");
    expect(boundFilePath("vision\\src\\App.tsx")).toBe("vision\\src\\App.tsx");
  });

  it("rejects null and empty inputs", () => {
    expect(boundFilePath(null)).toBeNull();
    expect(boundFilePath(undefined)).toBeNull();
    expect(boundFilePath("")).toBeNull();
    expect(boundFilePath("   ")).toBeNull();
  });

  it("rejects POSIX absolute paths", () => {
    expect(boundFilePath("/etc/passwd")).toBeNull();
    expect(boundFilePath("/usr/local/bin/whatever")).toBeNull();
    expect(boundFilePath("/")).toBeNull();
  });

  it("rejects Windows drive-letter absolute paths", () => {
    expect(boundFilePath("C:\\Windows\\System32\\config.sys")).toBeNull();
    expect(boundFilePath("c:/users/anish/secret.txt")).toBeNull();
    expect(boundFilePath("D:\\source\\file.rs")).toBeNull();
  });

  it("rejects Windows UNC paths", () => {
    expect(boundFilePath("\\\\server\\share\\file.rs")).toBeNull();
    expect(boundFilePath("\\\\?\\C:\\file.rs")).toBeNull();
  });

  it("rejects paths containing .. segments", () => {
    expect(boundFilePath("../etc/passwd")).toBeNull();
    expect(boundFilePath("src/../../escape.txt")).toBeNull();
    expect(boundFilePath("a\\..\\b")).toBeNull();
    expect(boundFilePath("..")).toBeNull();
  });

  it("does NOT reject filenames that merely contain dot dot", () => {
    // ".." as a SEGMENT is rejected; ".." as part of a filename
    // (e.g. "foo..bar.rs") is fine — that's a legitimate filename.
    expect(boundFilePath("foo..bar.rs")).toBe("foo..bar.rs");
    expect(boundFilePath("src/file..bak")).toBe("src/file..bak");
  });

  it("trims whitespace", () => {
    expect(boundFilePath("  src/lib.rs  ")).toBe("src/lib.rs");
    expect(boundFilePath("\tvision/foo.ts\n")).toBe("vision/foo.ts");
  });

  // Audit fix (2026-05-06 multi-agent fan-out, security-sentinel):
  // four bypass classes the original M-4 filter missed.

  it("rejects NUL byte (path truncation bypass)", () => {
    expect(boundFilePath("src/lib.rs\0/etc/passwd")).toBeNull();
    expect(boundFilePath("\0")).toBeNull();
    // After the trim() leading/trailing NULs would be stripped if
    // \0 counted as whitespace, but it doesn't — make sure we
    // catch interior NULs explicitly.
    expect(boundFilePath("src\0lib.rs")).toBeNull();
  });

  it("rejects other ASCII control characters (log injection)", () => {
    expect(boundFilePath("src/\x01lib.rs")).toBeNull();
    expect(boundFilePath("src/\x1blib.rs")).toBeNull();
    expect(boundFilePath("src/\x7flib.rs")).toBeNull();
  });

  it("rejects percent-encoded traversal markers", () => {
    expect(boundFilePath("src/%2e%2e/etc")).toBeNull();
    expect(boundFilePath("src/%2E%2E/etc")).toBeNull();
    expect(boundFilePath("src%2flib.rs")).toBeNull();
    expect(boundFilePath("src%5clib.rs")).toBeNull();
  });

  it("rejects fullwidth Unicode dot/slash (NFKC bypass)", () => {
    // U+FF0E fullwidth full stop + U+FF0F fullwidth solidus.
    expect(boundFilePath("src/．．/etc")).toBeNull();
    expect(boundFilePath("src／lib.rs")).toBeNull();
    expect(boundFilePath("．．")).toBeNull();
  });

  it("rejects lone `.` segments", () => {
    expect(boundFilePath("./src/lib.rs")).toBeNull();
    expect(boundFilePath(".")).toBeNull();
    expect(boundFilePath("src/./lib.rs")).toBeNull();
  });

  it("does NOT reject filenames that contain dot characters legitimately", () => {
    // Hidden files (start with .) are valid file names — but
    // ".env" is a single segment, not a "." segment, so it should
    // pass.
    expect(boundFilePath(".env")).toBe(".env");
    expect(boundFilePath("src/.gitignore")).toBe("src/.gitignore");
    // Mid-name dot is fine.
    expect(boundFilePath("README.md")).toBe("README.md");
  });
});
