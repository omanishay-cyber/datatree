# Library Evaluation — Deep Reference

> Systematic framework for evaluating, comparing, and selecting third-party libraries. Never pick a library on gut feeling — use data.

---

## Comparison Matrix Template

Use this template when evaluating 2-3 candidate libraries for the same purpose.

```
LIBRARY COMPARISON: [Purpose/Need]
===================================
Date: [YYYY-MM-DD]
Context: [What problem are we solving? What are the constraints?]

Candidates:
  A: [library-a] — [one-line description]
  B: [library-b] — [one-line description]
  C: [library-c] — [one-line description] (optional)

| Criterion        | Weight | Library A      | Library B      | Library C      |
|-----------------|--------|---------------|---------------|---------------|
| Bundle size     | High   | [size, gzip]  | [size, gzip]  | [size, gzip]  |
| Tree-shakeable  | High   | Yes/No        | Yes/No        | Yes/No        |
| Last publish    | High   | [date]        | [date]        | [date]        |
| Open issues     | Medium | [count]       | [count]       | [count]       |
| Contributors    | Medium | [count]       | [count]       | [count]       |
| TypeScript      | Medium | Built-in/DT   | Built-in/DT   | Built-in/DT   |
| Type quality    | Medium | Good/Fair/Poor| Good/Fair/Poor| Good/Fair/Poor|
| API simplicity  | Medium | [1-5 rating]  | [1-5 rating]  | [1-5 rating]  |
| Documentation   | Medium | [1-5 rating]  | [1-5 rating]  | [1-5 rating]  |
| GitHub stars    | Low    | [count]       | [count]       | [count]       |
| npm weekly DLs  | Low    | [count]       | [count]       | [count]       |
| License         | Low    | [license]     | [license]     | [license]     |
| Known CVEs      | Critical| [count]      | [count]       | [count]       |

WINNER: [Library X]
RATIONALE: [Why this one wins overall, considering weights]
TRADE-OFFS: [What we give up by not choosing the others]
```

---

## Red Flags (Automatic Disqualifiers)

These are signals that a library should be rejected or used with extreme caution:

### Critical Red Flags (Do Not Use)
- **Known unpatched CVEs** in the current version
- **License incompatible with commercial use** (GPL without linking exception, AGPL for non-server code)
- **Abandoned**: No commits in 2+ years AND unresolved critical issues
- **No error handling documentation** — suggests immature library

### Warning Red Flags (Proceed with Caution)
- **Single maintainer** with no succession plan (bus factor = 1)
- **No TypeScript support** — neither built-in types nor @types package
- **Excessive dependencies** — more than 20 transitive dependencies for a utility library
- **No tests in the repository** — suggests poor quality control
- **Breaking changes in minor versions** — check release history
- **README says "experimental" or "alpha"** for production-critical functionality
- **Last publish > 1 year ago** — check if it is stable-and-complete vs abandoned
- **GitHub stars < 100** for a library that would be a critical dependency
- **No changelog** — impossible to evaluate upgrade risk

---

## Green Flags (Positive Indicators)

### Strong Indicators
- **Active maintenance**: Regular releases, responsive to issues
- **Good documentation**: API reference, examples, migration guides
- **Built-in TypeScript types**: First-class TS support, well-typed generics
- **Small bundle size**: < 10KB gzipped for utilities, tree-shakeable
- **Zero or minimal dependencies**: Reduces supply chain risk
- **Comprehensive test suite**: High coverage, diverse test types
- **Semantic versioning**: Follows semver strictly, clear changelog

### Moderate Indicators
- **Used by well-known projects**: Validated at scale
- **Active community**: Discussions, third-party tutorials, Stack Overflow answers
- **Multiple maintainers**: Reduced bus factor
- **CI/CD pipeline visible**: Automated testing and publishing
- **Security policy**: SECURITY.md with responsible disclosure process

---

## Decision Process

### Step-by-Step

```
1. IDENTIFY NEED
   - What specific problem does this library solve?
   - Can we solve this with built-in APIs or existing dependencies?
   - Is this a "nice to have" or a critical dependency?

2. FIND CANDIDATES (2-3 maximum)
   - Search npm for the category
   - Check "alternatives" sections in library READMEs
   - Use WebSearch: "best [category] library [year]"
   - Ask: what does the existing codebase already use for similar needs?

3. EVALUATE AGAINST CRITERIA
   - Fill out the comparison matrix above
   - Check all red flags
   - Run: npm view [pkg] to get metadata
   - Check bundlephobia.com for size impact

4. TEST WITH SMALL POC (if close call)
   - Install the top 1-2 candidates
   - Implement the exact use case needed (not a generic hello world)
   - Evaluate: API ergonomics, error messages, edge cases
   - Time: maximum 30 minutes per candidate

5. DECIDE AND DOCUMENT
   - Pick the winner based on weighted criteria
   - Document WHY in the research findings report
   - Note what would make you reconsider (e.g., "if they add TS types")
```

### When NOT to Add a Library

Before adding any library, ask these questions:

```
- Can I do this with built-in Node/Browser APIs? (fetch, crypto, fs)
- Can I do this with a library already in package.json?
- Is this a <50 line utility I could write myself?
- Am I adding this library for one small feature? (just copy that function)
- Will this library's API surface outlive the need? (over-engineering)
```

If any answer is "yes," strongly consider NOT adding the library.

### The "1-Year Test"

Ask: "Will this library still be maintained and relevant in 1 year?"

- If YES with high confidence: safe to adopt
- If MAYBE: adopt but write an adapter/wrapper so you can swap later
- If NO or UNSURE: write it yourself or use a more established alternative
