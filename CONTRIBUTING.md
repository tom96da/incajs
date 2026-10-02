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
- Use `pnpm` for all JS tooling. Vite's supported runtime is Node.js, and the
  HMR bridge builds on its Runtime API, so avoid a second runtime such as Bun.

## Rules

- **License**: dual-licensed MIT OR Apache-2.0
  ([LICENSE-MIT](./LICENSE-MIT), [LICENSE-APACHE](./LICENSE-APACHE)). Start
  each new source file with:
  ```
  Copyright (c) <year> tom96da
  SPDX-License-Identifier: MIT OR Apache-2.0
  ```
- **Commits**: follow [GIT.md](./specs/GIT.md) for the message format and
  review policy.
- **Checks**: run the lint, format, type-check and test commands in
  [TESTING.md](./specs/TESTING.md) before opening a pull request.

## Pull requests

Branch from `main` as `feature/<name>`, keep the change focused, and describe
what it does and why. Add a line to [CHANGELOG.md](./CHANGELOG.md) for any
user-visible change.

## More

[specs/DEVELOPMENT.md](./specs/DEVELOPMENT.md) covers the project status,
architecture and guiding principles. [specs/STRUCTURE.md](./specs/STRUCTURE.md)
maps the repository.
