/**
 * I-9: projectIdForPath must canonicalize before hashing so different
 * spellings of the same Windows path map to the same ProjectId.
 *
 * Run with: `bun test src/tools/__tests__/projectIdForPath.test.ts`
 */

import { describe, it, expect } from "bun:test";
import { projectIdForPath } from "../../store.ts";

describe("projectIdForPath canonicalization", () => {
  it("hashes mixed-case + mixed-slash variants of a Windows path identically", () => {
    if (process.platform !== "win32") {
      // The case-insensitive + backslash-normalization invariant is
      // Windows-only; on POSIX paths must remain case-sensitive.
      return;
    }

    // None of these need to exist on disk — realpath fails silently
    // and we fall through to the string-level normalizer.
    const variants = [
      "c:/users/user/x",
      "C:/Users/user/x",
      "C:\\Users\\User\\x",
    ];
    const ids = variants.map((p) => projectIdForPath(p));
    expect(ids[0]).toEqual(ids[1]);
    expect(ids[1]).toEqual(ids[2]);
  });

  it("returns a 64-char hex SHA-256 digest", () => {
    const id = projectIdForPath("/some/path/that/likely/does/not/exist");
    expect(id).toMatch(/^[0-9a-f]{64}$/);
  });

  it("on POSIX, case differences yield different ids", () => {
    if (process.platform === "win32") return;
    const a = projectIdForPath("/Users/x");
    const b = projectIdForPath("/users/x");
    expect(a).not.toEqual(b);
  });
});
