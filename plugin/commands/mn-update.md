---
name: /mn-update
description: Update the mneme installation — checks for a new version, downloads, swaps in place, restarts the daemon.
command: mneme update
---

# /mn-update

Update the mneme installation in-place. Network access is opt-in and
limited to a single user-configured URL (see CLAUDE.md "local-only
invariant"). The default flow checks the configured update channel,
downloads the next version, verifies the signature, and restarts the
supervisor.

## Usage

```
/mn-update --check                     # check only, do not download
/mn-update                              # check + download + apply (interactive)
/mn-update --to <version>              # pin a specific version
/mn-update --channel stable            # switch update channel
/mn-update --rollback                  # alias for /mn-rollback
```

## What this does

1. Reads `~/.mneme/config.toml` for the configured update URL.
2. Polls the URL for the latest version manifest.
3. If a newer version is available, downloads + verifies + stages.
4. Stops the supervisor, swaps binaries, restarts.
5. Records the change in `~/.mneme/install-receipts/`.

## When to use

- Quarterly to pick up scanner + parser improvements.
- When a security advisory lands for a bundled crate.

## When to skip

- Network-restricted environments (the project is local-first by design).
- Mid-sprint — schedule updates between work sessions.

See also: `/mn-rollback` (revert to the previous version),
`/mn-doctor` (verify post-update health).
