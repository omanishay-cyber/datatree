# fireworks-research — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 6. Verification Gate

Before delivering research findings, verify ALL of the following:

```
[ ] Every finding has a source cited (not "I think" or "generally")
[ ] Every finding has a confidence level (HIGH/MEDIUM/LOW)
[ ] Confidence levels are justified (HIGH requires official source)
[ ] Recommendation is actionable (specific enough to implement)
[ ] Alternatives were considered (at least 1, ideally 2-3)
[ ] No contradictions between findings left unresolved
[ ] Time box was respected (or exceeded with justification)
[ ] Gaps and unknowns are explicitly stated
```

### Confidence Level Definitions

| Level | Definition | Required Evidence |
|-------|-----------|-------------------|
| HIGH | Verified fact from authoritative source | Official docs, source code, or 3+ independent sources agree |
| MEDIUM | Likely accurate but not fully verified | 2 unofficial sources agree, or 1 reputable source |
| LOW | Plausible but uncertain | Single source, inference, or conflicting information |

---

## 7. Anti-Premature-Completion

**"I researched it" is NOT done.**

Done means ALL of the following:
- Structured report produced with all required sections
- Every finding has a source and confidence level
- Recommendation is actionable and justified
- Alternatives were considered and rejection reasons documented
- Gaps and unknowns are explicitly stated

### Common Premature Completion Traps

- "I found an article about it" — Did you verify from a second source?
- "The docs say X" — Did you check if that applies to your version?
- "Stack Overflow says Y" — Is the answer recent? Does it have high votes?
- "It should work" — Did you verify with a test or code check?

---

## 8. 3-Strike Rule

If 3 different search queries return nothing useful, the question itself is likely wrong.

### Recovery Process

```
Strike 1: Rephrase the question using different terminology
Strike 2: Broaden the scope (maybe the answer is in a related domain)
Strike 3: STOP SEARCHING. Step back and ask:
  - Is the question based on a false assumption?
  - Am I searching for the wrong thing entirely?
  - Should I ask the user for clarification?
  - Can I test/experiment instead of researching?
```

### After 3 Strikes

1. Report what was searched and what was found (even if unhelpful)
2. State the hypothesis about why searches failed
3. Suggest alternative approaches (testing, asking user, different framing)
4. Do NOT keep searching in a loop — that wastes time and context

---

## 9. Evidence Freshness Scoring

Weight sources by recency:

| Age | Freshness Score | Action |
|---|---|---|
| < 30 days | 1.0 | Trust fully |
| 30-90 days | 0.8 | Trust with verification |
| 90-180 days | 0.5 | Verify before using |
| > 180 days | 0.3 | Cross-reference with current source |
| > 1 year | 0.1 | Treat as historical context only |

---

## 10. Timestamped Artifact Directories

When dispatching parallel research agents, write outputs to:
`~/.claude/sessions/artifacts/{YYYY-MM-DD_HHMMSS}/`
- Each agent writes its report there
- Enables historical comparison across research sessions
- Enables post-mortem analysis of agent quality

---

## 11. Phase 4 Empirical Validation

After any research agent reports findings:
- Agent says "library X supports feature Y" → verify with `npm info` or docs
- Agent says "API endpoint returns Z" → actually call it
- Agent says "file exists at path" → check with Glob
- NEVER trust research claims without verification

---

## 12. Reference Links

### Internal References
- `references/codebase-mapping.md` — Architecture mapping, dependency graphing, data flow tracing
- `references/library-evaluation.md` — Comparison matrix template, red/green flags, decision process
- `references/report-format.md` — Full report template, confidence scoring, cross-referencing rules

### External Tools
- **Context7** — Library documentation queries (resolve-library-id + query-docs)
- **WebSearch** — Current information, comparisons, known issues
- **WebFetch** — Detailed page content extraction
- **bundlephobia.com** — Package bundle size analysis
- **npm trends** — Download trend comparisons
- **npx madge** — Dependency graph visualization

### Methodology Foundations
- Structured Analytical Techniques for Intelligence Analysis (Heuer & Pherson)
- Evidence-Based Software Engineering (Kitchenham et al.)
- The Art of Readable Code (Boswell & Foucher) — for codebase reading techniques

### Related Skills
- `fireworks-patterns` — code reading protocol
- `fireworks-architect` — research informs architecture
- `fireworks-estimation` — research reduces estimation uncertainty

---

## Scope Boundaries

- **MINIMUM**: Always consult at least 3 sources.
- **MAXIMUM**: Time-box research per type (quick: 15min, standard: 30min, deep: 60min).
