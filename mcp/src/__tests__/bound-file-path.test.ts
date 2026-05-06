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
});
