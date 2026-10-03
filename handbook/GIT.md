<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Git workflow

Conventions and hard rules for commits and other git operations in this
repository.

## Branching

GitHub flow. Work lands on `main` through short-lived `feature/<name>`
branches, and urgent fixes through `hotfix/<name>` branches. Both branch from
`main` and merge back via review. Never commit directly to `main`. A release
is prepared on `release/<version>`.

CD runs when a push to `main` ends with a release commit and CI passes.

## Commit message format

Follows [Conventional Commits](https://www.conventionalcommits.org/), with
these rules:

- `type` is one of `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, 
  `test`, `build`, `ci`, `chore`, `revert`.
- Match the `type` and scope that `git log --oneline -- <path>` shows for
  the area you touch. A scope names the affected crate or package.
- `description` is imperative, lower-case, no trailing period.
- `body` is a bullet list of what changed and why.
  - Prefer what a user can see over which files or code changed.
  - Each bullet follows the description's style: lower-case unless the
    first word is a proper noun (a filename, package name, etc.), and no
    trailing period.
  - Keep the body to about 300 characters.
- A release commit's subject is `chore(release): vX.Y.Z`.

Example:

    fix(jsenv): print the class name in console output

    - print an instance of `Foo` as `Foo { a: 1 }`
    - keep a plain object as `{ a: 1 }`

## Commits by AI agents

- Show the exact staged diff and the exact commit message, and get explicit
  approval of that content before running `git commit`. This applies to
  `--amend` too.
- Prefer a new commit over amending. Amend only when asked, and only a commit
  that has not been pushed.
- A commit whose content an agent wrote carries a `Co-Authored-By:` trailer
  naming the model.
