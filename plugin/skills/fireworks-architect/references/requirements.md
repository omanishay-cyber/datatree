# Requirements Validation — Spec Templates and Checklists

## Overview

Requirements validation ensures that every feature request is complete, clear, testable, consistent, and feasible before any implementation begins. This prevents the most expensive class of bugs: building the wrong thing correctly.

---

## Validation Checklist

Every requirement must pass these five checks before it enters the implementation pipeline.

### 1. Completeness

- Every user story has acceptance criteria
- Edge cases are documented (empty state, error state, boundary values)
- Non-functional requirements are specified (performance, security, accessibility)
- Dependencies on other features or modules are identified
- Data requirements are defined (what is stored, how it is structured)

### 2. Clarity

- No ambiguous language: "should", "might", "could", "possibly" are replaced with "must", "will"
- Specific and measurable: "fast" becomes "under 200ms", "many" becomes a number
- Single interpretation: two developers reading the requirement would build the same thing
- Technical terms are defined or referenced
- UI requirements include mockups or wireframes when applicable

### 3. Testability

- Every requirement can be verified with a test or manual check
- Each acceptance criterion uses Given/When/Then format
- Expected outputs are specified for each input
- Error conditions are defined with expected behavior
- Performance requirements include measurement methodology

### 4. Consistency

- No contradicting requirements within the same feature
- No contradicting requirements across different features
- Consistent terminology throughout (same concept = same name everywhere)
- Consistent with existing application behavior and patterns
- Consistent with the project's technology constraints

### 5. Feasibility

- Technically possible within constraints (Electron, React, TypeScript, sql.js)
- Achievable within the available time and resources
- Does not require changes to fundamental architecture
- External dependencies (APIs, services) are available and reliable
- Performance requirements are achievable with the current stack

---

## Spec Template

Use this template for every feature specification:

```markdown
## Feature: [Name]

### Overview
[1-2 sentence description of what this feature does and why it matters]

### User Story
As a [user role], I want to [action] so that [benefit].

### Acceptance Criteria
- [ ] Given [context/precondition], when [action], then [expected result]
- [ ] Given [context/precondition], when [action], then [expected result]
- [ ] Given [context/precondition], when [action], then [expected result]

### Edge Cases
- What if [the input is empty]?
- What if [the network is unavailable]?
- What if [the data is corrupted or invalid]?
- What if [the user cancels mid-operation]?
- What if [concurrent access occurs]?

### Non-Functional Requirements
- **Performance**: [target, e.g., "page loads in under 500ms with 10,000 products"]
- **Security**: [constraints, e.g., "data encrypted at rest", "input validated"]
- **Accessibility**: [requirements, e.g., "keyboard navigable", "screen reader compatible"]
- **Compatibility**: [constraints, e.g., "works on Windows 10+"]

### UI/UX Requirements
- [Mockup link or description]
- [Theme support: light and dark]
- [Responsive behavior at different window sizes]
- [Animations and transitions]

### Data Model
- [Tables affected]
- [Fields added/modified]
- [Migration required: yes/no]

### Dependencies
- Requires: [other features/modules that must exist first]
- Affects: [other features/modules that will be impacted]

### Technical Notes
- [Architecture considerations]
- [Libraries or tools needed]
- [Known limitations or trade-offs]

### Complexity Estimate
- [ ] Trivial (1 file, obvious)
- [ ] Simple (2-3 files, clear path)
- [ ] Medium (5+ files, some unknowns)
- [ ] Complex (system-wide impact)
- [ ] Critical (data/security implications)
```

---

## Requirements Review Process

### Before Implementation

1. **Author** writes the spec using the template above
2. **Reviewer** checks against the 5 validation criteria (completeness, clarity, testability, consistency, feasibility)
3. **User** (the user) approves the spec with explicit sign-off
4. **Architect** (Claude) estimates complexity and creates the implementation plan

### Red Flags During Review

- "TBD" or "TODO" in the spec — incomplete, do not proceed
- No acceptance criteria — untestable, do not proceed
- Ambiguous language without metrics — unclear, ask for specifics
- Dependencies not identified — risk of blocked work
- No edge cases documented — risk of fragile implementation
- "Make it like [competitor]" without specifics — requires clarification

---

## Requirements Anti-Patterns

### "The Kitchen Sink"
Adding requirements during implementation. Every new requirement goes through the full validation process, even if it seems small.

**Fix**: Scope lock. New ideas are captured as separate stories for future sprints.

### "The Obvious Requirement"
Assuming requirements are so obvious they do not need writing down. These are the ones most likely to be misunderstood.

**Fix**: Write it down anyway. Even a one-line acceptance criterion is better than none.

### "The Gold Plate"
Over-specifying implementation details. Requirements should describe WHAT, not HOW. Let the architect decide the implementation approach.

**Fix**: Focus on user-visible behavior and measurable outcomes, not code structure.

### "The Moving Target"
Requirements that change daily. Frequent changes indicate the problem is not well understood yet.

**Fix**: Return to research phase. Interview the user. Understand the real problem before specifying the solution.

---

## Quick Reference: Given/When/Then Examples

```
Given the user is on the product list page,
When they click the "Add Product" button,
Then a modal opens with an empty product form.

Given the user has filled in all required fields,
When they click "Save",
Then the product is created in the database and appears in the product list.

Given the user submits the form with a duplicate SKU,
When the save operation fails,
Then an error message "SKU already exists" is displayed and the form remains open.

Given the database contains 10,000 products,
When the user opens the product list,
Then the first page of results loads within 500ms.

Given the user is editing a product in one window,
When another process updates the same product,
Then the user is notified of the conflict before saving.
```

---

## Linking Requirements to Implementation

Every step in an implementation plan should trace back to a specific requirement:

```
Step 1: [File: src/main/handlers/products.ts]
  Requirement: "Given the user has filled in all required fields, when they click Save..."
  Change: Add ipcMain.handle('product:create') with Zod validation

Step 2: [File: src/renderer/components/ProductForm.tsx]
  Requirement: "Then a modal opens with an empty product form"
  Change: Create ProductForm component with required fields
```

This traceability ensures no step is arbitrary and no requirement is forgotten.
