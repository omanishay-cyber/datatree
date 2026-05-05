# Layer 1 — UserPromptSubmit

Fires on every user message. Classifies into three intents and injects the appropriate context reminder.

## Intent classifier

| Intent | Trigger | Heavy injection? |
|---|---|---|
| **Resume** | Prompt ≤ 40 chars AND contains one of: `continue`, `resume`, `where was i`, `where were we`, `carry on`, `keep going`, `proceed`. | YES — `mneme_resume`, `step_status`, `step_show` |
| **Code** | Long prompt OR contains code-shaped tokens (`fn`, `function`, `class`, `import`, file paths, `::`, etc.). | LIGHT — top-3 tool reminder + grep/read trespass log |
| **Simple** | Short, no code keywords, no resume signal. | None — empty injection |

The 40-char length cap on Resume prevents misclassification. A user saying "resume training the dog" has the word `resume` but isn't asking the AI to continue a coding task. The 40-char gate eliminates these false positives.

The Simple intent emits NO injection because:

- Conversational prompts shouldn't drown in injected context
- The user is having a conversation, not directing a refactor
- The AI already knows about Mneme from the system prompt

## Output shape

```json
{
  "hook_specific": {
    "decision": "approve",
    "additionalContext": "..."
  }
}
```

The `additionalContext` field is what the AI actually sees. Heavy injection is ~2 KB; light injection is ~500 B; Simple injection omits the field entirely.

## Source

[`cli/src/commands/userprompt_submit.rs`][src] — 370 LOC + 7 tests covering each intent + edge cases.

[src]: https://github.com/omanishay-cyber/guess-test/blob/main/cli/src/commands/userprompt_submit.rs

## Configuration

```toml
[hooks]
inject_user_prompt_reminder = true   # default true
```

Set to `false` to disable Layer 1 entirely.
