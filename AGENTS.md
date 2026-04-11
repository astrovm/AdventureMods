## Approach

- Think before acting.
- Read files before editing them. Do not edit blind.
- Prefer small edits over rewrites.
- Do not re-read files unless they may have changed.
- Probe before fixing when the issue depends on runtime behavior, live data, downloaded assets, or external services.
- Treat unproven concerns as risks, not bugs. If you have not reproduced it or probed it, say so plainly.
- Test before declaring work done.
- Keep solutions and responses simple, direct, and concise.
- User instructions always override this file.

## Output

- Return code first. Explanation after, only if non-obvious.
- No inline prose. Use comments only when the logic is not obvious.
- No boilerplate unless explicitly requested.

## Code Rules

- Simplest working solution. No over-engineering.
- No abstractions for single-use operations.
- No speculative features or "you might also want..."
- When touching code, clean up the immediate area: remove unused variables, parameters, imports, helper functions, and dead branches.
- Do not leave duplication behind when a small local simplification removes it cleanly.
- Simplify changed code until the remaining logic is the smallest clear version that preserves behavior.
- No docstrings or type annotations on code not being changed.
- No error handling for scenarios that cannot happen.
- Three similar lines is better than a premature abstraction.
- Add new imports at the top of the file, not inside functions.
- Remove dead code and unused imports, variables, constants, and functions immediately.

## Review Rules

- State the bug. Show the fix. Stop.
- No suggestions beyond the scope of the review.
- No compliments or filler.

## Debugging Rules

- Read the relevant code before explaining the bug.
- Prove the bug with direct evidence first: a failing test, a reproduced run, a live archive listing, API metadata, or another concrete probe.
- If a claim depends on external content, inspect the actual content when feasible. Do not infer from names, descriptions, or assumptions.
- State what you found, where, and the fix.
- If the cause is unclear, say so. Do not guess.

## Verification Rules

- Use the project's actual tools for verification, not generic statements.
- Run the smallest command that proves the specific issue first, then run the broader repo checks needed for the touched area.
- For Rust code changes, default to these checks unless the change clearly does not require one of them:
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- If you skip a check, state which one and why.
- Do not claim a bug is fixed, a refactor is safe, or a release is ready without fresh command output.

## Git Rules

- Merge to `main` with a single squashed commit only.

## Simple Formatting

- No em dashes, smart quotes, or decorative Unicode symbols.
- Plain hyphens and straight quotes only.
- Natural language characters (accented letters, CJK, etc.) are fine when the content requires them.
- Code output must be copy-paste safe.
