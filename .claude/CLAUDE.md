## Coding

### About comment

Do not write comments explaining the code as you write it. Comments should be used in only two cases: when necessary for documentation, or for parts of the code that are non-standard or difficult to understand.

## Committing

- Before committing, make sure `just test` task to be passed. This task runs all builds and tests for the all packages.

## Creating PRs

- PR title format: `<type>(<scope>): <subject>`
  - Type follows the conventional commit format. (e.g. feat, fix, refactor, chore, etc.)
  - Scope is the name of the package or module that the commit affects, which is the package's directory name (e.g. `core`, `node`, `remote`). Packages are declared in the root `xtask.config.ts`.
    - If this PR does not affect any package, leave it empty. (e.g. `chore: update dependencies`)
  - Subject should be a short description of the change, no more than 50 characters.
- PR description follows the [repo PR template](https://github.com/webview-bundle/webview-bundle/blob/main/.github/PULL_REQUEST_TEMPLATE.md)
- Avoid being verbose; keep your input brief and clear, focusing only on the core issue being resolved.
