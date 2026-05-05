// mcp/src/__tests__/userprompt-submit.test.ts
//
// Coverage for the smart-context-injection classifier introduced in
// Item #119 (2026-05-05). Three tiers: simple → no injection;
// code → light block; resume → heavy block. Each axis must NOT
// drift across reasonable variations of user phrasing.

import { describe, it, expect } from "bun:test";
import { classifyPromptIntent } from "../hooks/userprompt-submit.ts";

describe("classifyPromptIntent — simple acks", () => {
  it.each([
    "ok thanks",
    "thanks!",
    "got it",
    "great",
    "nice",
    "what next",
    "hello",
    "how are you",
    "remind me what we agreed on",
    "any thoughts on this design?",
  ])("'%s' → simple", (prompt) => {
    expect(classifyPromptIntent(prompt)).toBe("simple");
  });
});

describe("classifyPromptIntent — code intent", () => {
  it.each([
    "find all callers of WorkerPool::spawn",
    "rewrite the audit function",
    "edit cli/src/main.rs and add a flag",
    "what's the type of EdgeKind?",
    "show me the bug in build.rs",
    "search for trait impls",
    "blast radius of changing PathManager",
    "trace who calls mneme-daemon",
    "compile this on windows",
    "fix the typescript file at vision/src/api.ts",
    "where is fetchEdges defined",
  ])("'%s' → code", (prompt) => {
    expect(classifyPromptIntent(prompt)).toBe("code");
  });
});

describe("classifyPromptIntent — resume cues", () => {
  it.each([
    "continue",
    "where was i",
    "where were we",
    "resume",
    "carry on",
    "keep going",
    "proceed",
    "continue, please",
    "resume from where we left off",
  ])("'%s' → resume", (prompt) => {
    expect(classifyPromptIntent(prompt)).toBe("resume");
  });

  it("long prompts that include 'continue' do NOT trigger resume", () => {
    // Length gate prevents "continue editing the WorkerPool spawn function
    // in manager.rs" (which has clear code intent) from being misclassified
    // as resume.
    expect(
      classifyPromptIntent(
        "continue editing the WorkerPool spawn function in manager.rs and add error handling",
      ),
    ).toBe("code");
  });
});
