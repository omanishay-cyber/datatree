---
name: /mn-rollback
description: Roll the mneme installation back to a previous version, or roll a project's shards back to a previous snapshot.
command: mneme rollback
---

# /mn-rollback

Revert mneme to an earlier state. Two modes:

- **Install rollback** — swap binaries back to the previous version
  recorded in `~/.mneme/install-receipts/`.
- **Snapshot rollback** — restore a project's shards from a snapshot
  created by `/mn-snap`.

## Usage

```
/mn-rollback                           # auto-detect: most recent install or snap
/mn-rollback --install                 # rollback the binary install
/mn-rollback --to-snap <snap-id>       # restore project shards to a snapshot
/mn-rollback --to-version <semver>     # rollback to a specific binary version
/mn-rollback list                      # list available rollback targets
```

## What this does

For an install rollback:

1. Reads the install receipts to find the previous version.
2. Stops the supervisor.
3. Restores the previous binaries from `~/.mneme/install-cache/`.
4. Restarts the supervisor.

For a snapshot rollback:

1. Acquires the BuildLock for the project.
2. Stops indexers.
3. Replaces the live shards with hard-links from the snapshot directory.
4. Releases the lock; resumes indexing on the new baseline.

## When to use

- A new release introduced a regression you can't ship around.
- An experimental refactor poisoned the local index — restore a clean
  snapshot taken before the experiment.

See also: `/mn-snap` (capture a snapshot first), `/mn-update`
(forward-only update), `/mn-doctor` (verify post-rollback state).
