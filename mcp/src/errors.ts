/**
 * Bug TS-5 (2026-05-01): safe error-message extractor.
 *
 * The pattern `(err as Error).message` is wrong when `catch (err)` is
 * given a non-Error throw value (a string, a number, null, an object
 * literal). Accessing `.message` on those returns `undefined`, which
 * propagates into user-facing error strings as the literal text
 * "spawn failed: undefined" — the user sees a useless error and we
 * lose the actual failure cause.
 *
 * `errMsg` handles all the common cases:
 *   - Real Error instance → return its `.message`
 *   - String throw         → return the string directly
 *   - Object with .message → return that
 *   - Anything else        → JSON.stringify (truncated) or "[unknown]"
 *
 * Use this everywhere instead of `(err as Error).message`.
 */
export function errMsg(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  if (err === null) return "null";
  if (err === undefined) return "undefined";
  if (typeof err === "object") {
    const maybe = err as { message?: unknown };
    if (typeof maybe.message === "string") return maybe.message;
    try {
      const s = JSON.stringify(err);
      return s.length > 500 ? `${s.slice(0, 500)}…` : s;
    } catch {
      return "[unstringifiable error]";
    }
  }
  return String(err);
}
