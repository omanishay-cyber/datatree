# Git Workflow — Deep Reference

> Advanced git operations: merge vs rebase, conflict resolution, interactive rebase, stash patterns, cherry-pick, and bisect.

---

## Merge vs Rebase

### When to Merge

```
Use MERGE for:
  - Merging feature branches into main (preserves branch history)
  - Merging hotfix branches into main
  - Any merge that goes into a shared/protected branch
  - When you want to preserve the exact history of development

Command:
  git checkout main
  git merge feat/my-feature

Result: Creates a merge commit, preserves branch topology
```

### When to Rebase

```
Use REBASE for:
  - Keeping your feature branch up to date with main
  - Cleaning up messy commit history BEFORE creating a PR
  - Applying your changes on top of the latest main

Command:
  git checkout feat/my-feature
  git rebase main

Result: Moves your commits to the tip of main, linear history

DANGER: NEVER rebase commits that have been pushed and shared.
  - Rebase rewrites commit hashes
  - Other developers' branches will break
  - Only rebase LOCAL, unpushed commits
```

### Decision Flowchart

```
Is the branch shared with others?
  YES → MERGE (never rebase shared branches)
  NO → Continue...

Is this going INTO main?
  YES → MERGE (preserve the feature branch context)
  NO → Continue...

Are you updating your feature branch FROM main?
  YES → REBASE (keep your branch clean and up to date)
  NO → Continue...

Are you cleaning up before a PR?
  YES → REBASE (squash and clean up commit history)
  NO → MERGE (safe default)
```

---

## Conflict Resolution

### Understanding Conflicts

```
<<<<<<< HEAD (your changes)
const price = quantity * unitPrice;
=======
const price = quantity * unitCost * (1 + margin);
>>>>>>> feat/profit-margin (incoming changes)
```

### Resolution Process

```
1. UNDERSTAND both sides:
   - Read the conflicting sections carefully
   - Understand WHY each change was made
   - Check the commit messages for context

2. DECIDE the correct resolution:
   - Sometimes one side is clearly correct
   - Sometimes you need to combine both changes
   - Sometimes neither is correct and you need a new approach

3. RESOLVE the conflict:
   - Remove the conflict markers (<<<<, ====, >>>>)
   - Write the correct code
   - Ensure the result compiles and passes tests

4. TEST after resolving:
   - Run tsc --noEmit
   - Run relevant tests
   - Check the UI if applicable

5. NEVER:
   - Just accept one side blindly ("accept theirs" without reading)
   - Leave conflict markers in the code
   - Skip testing after resolution
```

### Complex Conflict Strategy

```
For conflicts spanning many files:
  1. git status — see all conflicted files
  2. Start with the smallest/simplest conflicts
  3. Work up to the most complex
  4. Test after EACH file is resolved
  5. git add <resolved-file> after each resolution
  6. git rebase --continue (or git merge --continue)

If it gets too hairy:
  git rebase --abort  (or git merge --abort)
  Discuss with the team / user before trying again
```

---

## Interactive Rebase (Pre-PR Cleanup)

Use interactive rebase to clean up commit history BEFORE creating a PR. Never during review.

### Common Operations

```bash
# Rebase last N commits interactively
# NOTE: Claude Code cannot use -i flag (requires interactive input)
# Instead, use non-interactive methods:

# Squash last 3 commits into one
git reset --soft HEAD~3
git commit -m "feat(inventory): add barcode scanner support"

# Or use fixup commits during development:
git commit --fixup=<commit-hash>
git rebase --autosquash main
```

### When to Clean Up

```
CLEAN UP when:
  - You have "WIP" or "fix typo" commits
  - Multiple small commits that form one logical change
  - Commit messages don't follow conventions
  - You want a clean, reviewable PR

DO NOT CLEAN UP when:
  - Commits are already pushed and reviewed
  - Each commit represents a distinct, meaningful change
  - You're in the middle of a review (rewriting history confuses reviewers)
```

---

## Stash Patterns

### Basic Stash

```bash
# Stash all changes with a descriptive message
git stash save "WIP: halfway through invoice refactor"

# List all stashes
git stash list

# Apply most recent stash (keep in stash list)
git stash apply

# Apply and remove from stash list
git stash pop

# Apply a specific stash
git stash apply stash@{2}

# Drop a specific stash
git stash drop stash@{2}

# Clear all stashes (DESTRUCTIVE)
git stash clear
```

### Advanced Stash

```bash
# Stash only staged changes
git stash --keep-index

# Stash including untracked files
git stash --include-untracked

# Stash specific files (Git 2.13+)
git stash push -m "stash only config" -- src/config.ts

# Create a branch from a stash
git stash branch new-branch-name stash@{0}

# Show what's in a stash without applying
git stash show -p stash@{0}
```

### Stash Best Practices

```
- Always use descriptive messages: git stash save "WIP: reason"
- Don't accumulate stashes — apply or drop promptly
- Prefer branches over stashes for longer-lived work
- Use --include-untracked if you have new files
- Check git stash list before stashing (avoid losing track)
```

---

## Cherry-Pick

### When to Cherry-Pick

```
USE cherry-pick for:
  - Applying a specific hotfix to multiple release branches
  - Pulling a single commit from a feature branch (e.g., a bug fix)
  - Backporting a fix to an older version

DO NOT cherry-pick for:
  - Moving entire feature branches (use merge or rebase)
  - Regular workflow (creates duplicate commits)
```

### How to Cherry-Pick

```bash
# Cherry-pick a single commit
git cherry-pick <commit-hash>

# Cherry-pick without committing (stage changes only)
git cherry-pick --no-commit <commit-hash>

# Cherry-pick a range of commits
git cherry-pick <start-hash>..<end-hash>

# If conflicts occur
git cherry-pick --continue  # after resolving conflicts
git cherry-pick --abort     # to cancel

# Cherry-pick with a reference to original commit
git cherry-pick -x <commit-hash>  # adds "cherry picked from" to message
```

---

## Bisect (Finding Regression Commits)

### When to Bisect

```
Use bisect when:
  - A bug exists now but didn't exist before
  - You don't know which commit introduced the bug
  - There are many commits between "working" and "broken"
  - Manual inspection would take too long

Bisect uses binary search — O(log n) instead of O(n)
  100 commits → ~7 tests
  1000 commits → ~10 tests
```

### How to Bisect

```bash
# Start bisecting
git bisect start

# Mark current commit as bad (has the bug)
git bisect bad

# Mark a known good commit (before the bug)
git bisect good <commit-hash>

# Git checks out a commit halfway between good and bad
# Test the application, then:
git bisect good  # if this commit doesn't have the bug
git bisect bad   # if this commit has the bug

# Repeat until git identifies the first bad commit
# Git will say: "<hash> is the first bad commit"

# When done, return to your branch
git bisect reset
```

### Automated Bisect

```bash
# Run a test script automatically
git bisect start
git bisect bad HEAD
git bisect good v1.0.0
git bisect run npm test  # or any script that exits 0 for good, 1 for bad

# Git will automatically find the first bad commit
```

### Bisect Tips

```
- Before starting: make sure you can reliably reproduce the bug
- Have a simple test (manual or automated) that clearly shows good vs bad
- If a commit doesn't compile: git bisect skip
- Save the bisect log: git bisect log > bisect.log
- Replay a saved bisect: git bisect replay bisect.log
```
