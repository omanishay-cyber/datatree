# Report Format — Deep Reference

> Templates, examples, and guidelines for producing structured research findings. Every research task ends with a report — no exceptions.

---

## Full Report Template

```
RESEARCH FINDINGS
=================
Topic: [Precise description of what was researched]
Date: [YYYY-MM-DD]
Time spent: [X minutes]
Scope: [What was included in research / what was excluded]
Trigger: [What prompted this research — user question, bug, architecture decision]

SOURCES CONSULTED:
  1. [Source name] — [URL or file path] — [authority level: Official/Community/Inference]
  2. [Source name] — [URL or file path] — [authority level]
  3. [Source name] — [URL or file path] — [authority level]

KEY FINDINGS:
  1. [Concise finding statement]
     Confidence: HIGH
     Source: [Official docs — URL]
     Detail: [Additional context if needed]

  2. [Concise finding statement]
     Confidence: MEDIUM
     Source: [GitHub issue #123, Stack Overflow answer]
     Detail: [Additional context if needed]

  3. [Concise finding statement]
     Confidence: LOW
     Source: [Single blog post, inference]
     Detail: [Why confidence is low, what would raise it]

CONTRADICTIONS FOUND:
  - [Source A says X, but Source B says Y]
    Resolution: [Which is correct and why, or "unresolved"]

RECOMMENDATION:
  [Clear, actionable recommendation — what to do next]
  Rationale: [Why this recommendation over alternatives]
  Risk: [What could go wrong with this recommendation]

ALTERNATIVES CONSIDERED:
  A. [Alternative approach]
     Pros: [list]
     Cons: [list]
     Why rejected: [specific reason]

  B. [Alternative approach]
     Pros: [list]
     Cons: [list]
     Why rejected: [specific reason]

GAPS / UNKNOWNS:
  - [What couldn't be determined]
  - [What needs further investigation]
  - [Assumptions made that should be validated]

NEXT STEPS:
  1. [Concrete, actionable step with owner]
  2. [Concrete, actionable step with owner]
  3. [Concrete, actionable step with owner]
```

---

## Short Report Template

Use for simple lookups and quick evaluations (< 10 minutes of research).

```
QUICK FINDINGS: [Topic]
========================
Sources: [1-3 sources]

FINDINGS:
  1. [Finding] — Confidence: HIGH/MEDIUM/LOW
  2. [Finding] — Confidence: HIGH/MEDIUM/LOW

RECOMMENDATION: [What to do]
```

---

## Confidence Scoring Guide

### HIGH Confidence Requirements (at least ONE of these)

- Verified from official documentation (library docs, MDN, Node.js docs, RFCs)
- Verified from source code (you read the actual implementation)
- Confirmed by 3+ independent sources that all agree
- Confirmed by library maintainer (in GitHub issue, blog post, or docs)
- Reproducible: you tested it and observed the behavior directly

### MEDIUM Confidence Requirements (at least ONE of these)

- 2 independent unofficial sources agree (blog posts, tutorials, SO answers)
- 1 reputable source (MDN, web.dev, well-known author's blog)
- Official docs from a related but not identical version
- Highly-voted Stack Overflow answer (50+ votes, not outdated)
- Consistent with observable behavior but not explicitly documented

### LOW Confidence Requirements (any of these)

- Single unverified source (one blog post, one SO answer with few votes)
- Inference from related information (not directly stated anywhere)
- Conflicting information from multiple sources (unresolved)
- Outdated source (> 2 years old for a fast-moving technology)
- AI-generated content without external verification

---

## Cross-Referencing Protocol

### When to Cross-Reference

- Always cross-reference before assigning HIGH confidence
- Always cross-reference when the finding affects architecture decisions
- Always cross-reference when the finding contradicts expectations
- Skip cross-referencing for trivial lookups (API syntax, config options)

### How to Cross-Reference

```
1. Find the primary source (usually official docs)
2. Search for at least 1 independent secondary source
3. Check for contradictions between sources
4. If contradictions found:
   a. Check which source is more authoritative
   b. Check which source is more recent
   c. Check if one source has a known bias
   d. If still unclear, report the contradiction and downgrade confidence
5. If sources agree: upgrade confidence
6. If no secondary source found: note as single-source finding
```

---

## When to Stop Researching

### Stop Immediately When

1. **Answer found with HIGH confidence** from official docs
   - No need to keep searching — you have the authoritative answer
   - Exception: if the answer seems wrong or surprising, verify once more

2. **Time box exceeded**
   - Report what you have with honest confidence levels
   - Note what would need more time to investigate
   - Let the user decide if more research time is warranted

3. **3 search reformulations failed** (3-Strike Rule)
   - The question may be wrong — report what you tried
   - Suggest alternative framings or approaches
   - Consider testing instead of researching

4. **Diminishing returns**
   - New sources just confirm what you already know
   - No new information in the last 3 searches
   - Confidence is already HIGH on all findings

### Keep Researching When

- Confidence is LOW on a critical finding
- Contradictions remain unresolved
- The recommendation depends on an unverified assumption
- The user explicitly asked for a thorough investigation
- The decision is high-impact and hard to reverse

---

## Report Quality Checklist

Before delivering any research report, verify:

```
COMPLETENESS:
  [ ] Every finding has a source cited
  [ ] Every finding has a confidence level
  [ ] Recommendation is present and actionable
  [ ] At least one alternative was considered
  [ ] Gaps and unknowns are explicitly stated

ACCURACY:
  [ ] HIGH confidence findings are verified from official sources
  [ ] No contradictions left unresolved without explanation
  [ ] Version numbers checked (findings apply to our version)
  [ ] Dates checked (findings are current, not outdated)

ACTIONABILITY:
  [ ] Recommendation is specific enough to implement
  [ ] Next steps are concrete (not "investigate further")
  [ ] Risks are identified for the recommended approach
  [ ] Alternative approaches are documented for fallback
```
