<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Contributing

Thanks for helping with Incarnative.js. Bug reports, fixes and docs
improvements are welcome. For a larger change, open an issue first so the
approach can be agreed before you write code.

## Setup

- Use the dev container (`.devcontainer/`). It carries the native
  dependencies `gpui` needs.
- Use `pnpm` (the version in `package.json`'s `packageManager`) and Node.js
  22.18 or newer. Other runtimes such as Bun are not tested.

## Rules

- **License**: dual-licensed MIT OR Apache-2.0
  ([LICENSE-MIT](./LICENSE-MIT), [LICENSE-APACHE](./LICENSE-APACHE)). Start
  each new source file with:
  ```
  Copyright (c) <year> tom96da
  SPDX-License-Identifier: MIT OR Apache-2.0
  ```
- **Commits**: follow [GIT.md](./handbook/GIT.md) for the message format.
- **Checks**: run the lint, format, type-check and test commands in
  [TESTING.md](./handbook/TESTING.md) before opening a pull request.

## Pull requests

Keep the change focused, and describe what it does and why. Branching follows
[GIT.md](./handbook/GIT.md#branching). Add a line to
[CHANGELOG.md](./CHANGELOG.md) for any user-visible change.

## More

[handbook/DEVELOPMENT.md](./handbook/DEVELOPMENT.md) covers the project
status, architecture and guiding principles. It links to the rest of the
handbook, including the repository map.
