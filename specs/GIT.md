<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Git workflow

Conventions and hard rules for commits and other git operations in this
repository. Anyone working here — AI agents included — follows these.

## Branching model

Just `main`, with short-lived `feature/<name>` branches merged
directly into it via review. Releasing is not a branch type — it's
whatever merged commit bumps `packages/core/package.json`'s version;
CD detects that on the push to `main` and tags/publishes from there.

Never commit directly to `main` — land work through a supporting
branch, merged in via review.

## Never commit without review

Committing (`git commit`, `git commit --amend`, or anything else that
creates/rewrites history) is never done unilaterally. Before running any
commit, show the exact staged diff and the exact final commit message, and
get explicit approval of that specific content — agreement that "committing
is the next step" in general is not the same as approval of the actual
diff/message. Preparing a commit and reporting it afterward is backwards;
review happens before the commit exists, not after.

## Amending vs. new commits

Prefer a new commit over amending. Amending is acceptable only when
explicitly requested, and only for a commit that hasn't been pushed anywhere
shared — the review rule above applies to amends exactly the same as to new
commits.

## Commit message format

Follows [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

- `type` is one of `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
- Before picking a `type`/`scope`, check existing precedent with `git log --oneline -- <path>` for the area being touched, and match it — e.g. this repo's devcontainer changes are `chore(devcontainer)`, not `build(devcontainer)`.
- `description` is imperative, lower-case, no trailing period (e.g. `feat(runtime): add quickjs bridge`).
- `body` is a concise bullet list of what was done and why — not prose. Each bullet follows the same style as the description: starts lower-case unless the first word is a proper noun (a filename, package name, etc.), and has no trailing period.
- A breaking change is marked either with `!` after the type/scope (`feat!: ...`) or a `BREAKING CHANGE:` footer — not both unless it aids clarity.
- Scope is optional; use it for the affected area once the workspace has named crates/packages (e.g. `fix(gpui-shell): ...`).
- Any commit Claude is involved in must include a `Co-Authored-By: Claude <noreply@anthropic.com>` trailer (adjust the model name if relevant, e.g. `Claude Sonnet 5`).
