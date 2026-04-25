---
name: /mn-graphify
description: Multimodal extraction pass (PDF / audio / video / images)
---

Run `mneme graphify <project>` and surface the results in this conversation.

When the user invokes `/mn-graphify`, you should:

1. Determine the project root: prefer the current workspace; fall back to CWD.
2. Spawn `mneme graphify` with appropriate flags (see below).
3. Capture stdout. Do NOT show raw stderr unless there's an error.
4. Format the result for the user.

## Args

- project path (optional, default CWD) — the directory to scan for multimodal assets.
- `--limit N` — cap the number of assets processed in a single pass.

## Example

```
$ mneme graphify .
scanning . for multimodal assets...
  pdf:    12 documents -> 84 concepts
  audio:   3 files     -> 21 concepts (whisper)
  images: 47 files     -> 110 concepts (ocr)
  video:   1 file      -> 14 concepts
graph updated: 229 new edges, 0 conflicts
```
