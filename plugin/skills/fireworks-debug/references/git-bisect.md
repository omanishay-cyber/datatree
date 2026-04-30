# Git Bisect — Regression Debugging

> When something that used to work is now broken, use git bisect to find
> the exact commit that introduced the regression in O(log n) tests.

---

## When To Use Git Bisect

- A feature worked before but is broken now
- You do not know which commit introduced the bug
- There are many commits between the known-good and known-bad states
- The bug is reproducible with a clear pass/fail test

---

## Performance Stats

Git bisect uses binary search. The number of tests needed:

| Commits to Search | Tests Required |
|-------------------|---------------|
| 10 | 4 |
| 50 | 6 |
| 100 | 7 |
| 500 | 9 |
| 1000 | 10 |

Formula: `ceil(log2(n))` tests for n commits.

---

## Manual Bisect Protocol

### Step 1: Start Bisect
```bash
git bisect start
```

### Step 2: Mark the Bad Commit (current state)
```bash
git bisect bad
# Or mark a specific commit:
git bisect bad HEAD
```

### Step 3: Mark a Known Good Commit
```bash
# Find a commit where the feature worked:
git bisect good v1.2.0
# Or use a specific hash:
git bisect good abc123
```

Git will now checkout a commit in the middle.

### Step 4: Test the Current Commit
1. Build and run the app: `npm install && npm run dev`
2. Test the feature that is broken
3. Mark the result:
```bash
# If the bug is present:
git bisect good

# If the bug is NOT present (feature works):
git bisect bad
```

Wait — that seems backwards. Remember: `good` means "this commit does NOT have the bug" and `bad` means "this commit HAS the bug."

### Step 5: Repeat
Git checks out the next commit to test. Repeat Step 4 until git identifies the first bad commit:
```
abc123 is the first bad commit
commit abc123
Author: Developer Name
Date: Mon Mar 15 2026

    feat: update product query to use new schema
```

### Step 6: End Bisect
```bash
git bisect reset
# Returns you to the branch you started on
```

### Step 7: Analyze the Culprit Commit
```bash
git show abc123
# See exactly what changed in the offending commit
```

---

## Automated Bisect with Test Script

If you can write a script that returns 0 for good and non-zero for bad:

```bash
# Create a test script: test-bisect.sh
#!/bin/bash
npm install --silent 2>/dev/null
npm run build 2>/dev/null
# Run a specific test or check
npm run test -- --testPathPattern="products" 2>/dev/null
# Exit code 0 = good (test passes), non-zero = bad (test fails)
```

```bash
# Run automated bisect:
git bisect start
git bisect bad HEAD
git bisect good v1.2.0
git bisect run ./test-bisect.sh
```

Git will automatically test each commit and find the first bad one.

---

## --first-parent for PR-Level Bisection

If you use merge commits (PR workflow), `--first-parent` restricts bisect to only merge commits on the main branch. This finds the PR that introduced the bug instead of the individual commit:

```bash
git bisect start --first-parent
git bisect bad HEAD
git bisect good v1.2.0
```

This is faster because it skips intermediate commits within PRs. Once you find the bad PR, you can bisect within that PR if needed.

---

## Integration with 10-Step Protocol

Git bisect fits into **Step 4: Binary Search** of the debugging protocol:

1. **Step 1 (Capture)**: Record the bug. Note when it was last known to work.
2. **Step 2 (Reproduce)**: Confirm the bug exists on the current commit.
3. **Step 3 (Trace)**: If the trace does not reveal the cause, move to bisect.
4. **Step 4 (Binary Search)**: Use git bisect instead of code-level binary search.
5. **Step 5 (Hypothesize)**: The bisect result tells you the exact commit. Form a hypothesis based on what changed.
6. Continue with Steps 6-10 as normal.

---

## Common Pitfalls

### Build Failures During Bisect
Some commits may not build (dependency changes, config changes). Skip them:
```bash
git bisect skip
```

### Database Schema Changes
If the database schema changed between good and bad, you may need to reset the database for each test. Include this in your test script.

### Node Modules Changes
Run `npm install` at each step. Dependencies may have changed.

### The Bug Is in a Dependency
If bisect points to a commit that only changed package.json, the bug is in a dependency update. Check the dependency changelog.

---

## Quick Reference

```bash
# Start
git bisect start
git bisect bad [bad-commit]
git bisect good [good-commit]

# At each step
git bisect good    # Bug NOT present
git bisect bad     # Bug IS present
git bisect skip    # Cannot test this commit

# Automated
git bisect run ./test-script.sh

# PR-level
git bisect start --first-parent

# Done
git bisect reset

# View bisect log
git bisect log
```
