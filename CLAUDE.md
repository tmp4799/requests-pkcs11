# Working Agreement

This file defines how you (the agent) work in this repo. It has no runtime — it is
read natively by the coding agent (Claude Code reads `CLAUDE.md`; other tools read the
mirror in `AGENTS.md`). There is nothing to install beyond `bd` (beads).

It distils the [ralphy](../ralphy) way of working into a lightweight, portable form:
the coding guidelines, a "keep me informed" habit, beads for task tracking, and the
ability to delegate to sub-agents.

---

## 1. Way of Working

These four guidelines bias toward caution over speed. For trivial tasks, use judgement.

### Think Before Coding
**Don't assume. Don't hide confusion. Surface tradeoffs.**

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them — don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### Simplicity First
**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### Surgical Changes
**Touch only what you must. Clean up only your own mess.**

- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it — don't delete it.
- Remove imports/variables/functions that YOUR changes made unused; leave pre-existing
  dead code alone unless asked.

The test: every changed line should trace directly to the request.

### Goal-Driven Execution
**Define success criteria. Loop until verified.**

- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan with a verify step for each line.

---

## 2. Keep Me Informed

I want to understand what you're doing and why, as you do it. This replaces ralphy's
heavyweight preview gate with a lightweight habit.

- **Before non-trivial work, state a brief plan and the reasoning** (a few lines: what
  you'll do, why this approach, what you're trading off). Trivial edits need no preamble
  — just make them.
- **Teach as you go.** When you use a non-obvious concept, technique, library, or
  tradeoff, explain it in a sentence or two. Assume I want to learn, not just receive a
  result.
- **Show the alternatives you rejected** when a decision was close, not only the path
  you took.
- **Report outcomes honestly.** If tests fail, say so with the output. If you skipped a
  step, say that. Don't claim done until it's verified.

The bar: after reading your update, I should be able to explain the change and the
reason to someone else.

---

## 3. Task Tracking with Beads

This project uses **bd (beads)** for issue tracking. Use it as the source of truth for
multi-step work — don't keep the task list only in your head or in chat.

Run `bd prime` once per session for the full, up-to-date workflow context.

**Quick reference:**
- `bd ready` — find unblocked work (start here each session)
- `bd create "Title" --type task --priority 2` — create an issue
- `bd show <id>` — read an issue's full detail
- `bd update <id> --status in_progress` — claim work before starting
- `bd close <id>` — complete work
- `bd dep <id> --blocks <other>` — record a dependency
- `bd dolt push` — push beads to the remote

**Habits:**
- At session start, run `bd ready` and tell me what's available before picking work.
- Break a request into beads when it has more than one verifiable step; link
  dependencies so `bd ready` stays accurate.
- Move a bead to `in_progress` when you start it and `close` it when its acceptance
  criteria are genuinely met — not before.
- Reference the bead ID in commit messages.
- Before creating, check `bd list` / `bd search` so you don't duplicate.

---

## 4. Delegating to Sub-Agents

For work that fans out or benefits from a fresh, focused context, delegate to a
sub-agent instead of doing everything in the main thread. This preserves ralphy's
multi-agent division of labour without the orchestrator.

The roles below live as native Claude Code subagents in `.claude/agents/`. If you're
running under a different tool (e.g. Codex) that doesn't load those files, spawn an
ad-hoc sub-agent and give it the same role and constraints described here.

| Role | When to use it | Access |
|------|----------------|--------|
| **architect** | Exploring a problem, designing an approach, or planning a change before any code is written. | Read-only — produces a plan, not edits. |
| **developer** | Implementing an approved plan or a well-scoped change, then running the project's checks. | Read/write. |
| **researcher** | Gathering facts, locating code across the repo, or answering a question that needs a wide search. | Read-only. |
| **reviewer** | Reviewing a diff for correctness bugs and quality/simplicity issues, and confirming tests pass. | Read-only. |

**How to delegate:**
- For a non-trivial change, prefer **architect → (you approve the plan) → developer →
  reviewer**. Tell me the plan before dispatching the developer.
- For a focused or low-risk change, go straight to **developer**, then **reviewer**.
- Use **researcher** whenever the answer needs a broad search and you only need the
  conclusion, not the file dumps.
- Sub-agents start with a clean context. Give each one everything it needs: the goal,
  the relevant files, the acceptance criteria, and the bead ID it's working under.
- Sub-agents must follow the guidelines in this file — restate the binding constraints
  in the task you hand them.


<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:6cd5cc61 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->
