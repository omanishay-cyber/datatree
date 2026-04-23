<!-- Thanks for contributing to mneme! Answer each section briefly. -->

## What changes
<!-- 1-2 sentences. What does this PR do? -->

## Why
<!-- What user problem or architectural gap does it address? -->

## How tested
<!-- Unit tests? Integration? Manual smoke test? Paste command output. -->

## Screenshots / recordings
<!-- If this changes the vision app, README, or any visible output, include a before/after. -->

## Checklist
- [ ] `cargo build --workspace` green
- [ ] `cargo clippy --workspace -- -D warnings` green
- [ ] `cargo test --workspace` green (new tests added for new behaviour)
- [ ] `cd mcp && bun test` green (if MCP touched)
- [ ] No new native DLL dependencies (if Rust side)
- [ ] `CHANGELOG.md` updated for user-facing changes
- [ ] Commit message follows Conventional Commits (feat/fix/docs/refactor/...)
- [ ] Linked issue (if any): closes #___

## Anything else
<!-- Breaking changes, follow-up tickets, rollback plan, etc. -->
