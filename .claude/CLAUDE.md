## Committing

- Before committing, make sure `just test` task to be passed. This task runs all builds and tests for the all packages.

## Creating PRs

- PR title format: `<type>(<scope>): <subject>`
  - Type follows the conventional commit format. (e.g. feat, fix, refactor, chore, etc.)
  - Scope is the name of the package or module that the commit affects. See the detailed list of scopes in each `xtask.config.json` file inside the package. (When it omitted, default to directory name)
    - If this PR does not affect any package, leave it empty. (e.g. `chore: update dependencies`)
  - Subject should be a short description of the change, no more than 50 characters.
- PR description follows the [repo PR template](https://github.com/webview-bundle/webview-bundle/blob/main/.github/PULL_REQUEST_TEMPLATE.md)
- Avoid being verbose; keep your input brief and clear, focusing only on the core issue being resolved.
