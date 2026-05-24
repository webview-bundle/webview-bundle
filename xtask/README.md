# xtask

A command-line tool for managing and automating tasks in a project.

## Features

Which packages changed is determined from the `package.json`/`Cargo.toml` **dependency graph**,
not conventional-commit scopes: a commit that touches files under a package's directory marks
that package changed, and every package that (transitively) depends on it is released too so it
picks up the new version. Contributors are not required to follow any commit convention.

### Releasing

1. A maintainer runs `just xtask prepare-release` locally. It lists, per package, the commits
   since its last tag (`<name>/<version>`), lets you pick which go in the changelog and the
   version bump, propagates through the dependency graph, then commits the bumps + changelogs to a
   content-addressed `release/<hash>` branch and opens (or updates) a PR via the `gh` CLI.
2. Merging that PR is the trigger: on the base branch the CI runs `just xtask release`, which
   publishes every package whose current version is not yet tagged, tags the merge commit, pushes
   the tags, and creates GitHub releases (uploading each package's configured assets).
   Tagging marks a package released, so a partial failure is recoverable — re-running republishes
   only the still-untagged packages.

### Pre-Releasing

On every other base-branch commit, CI runs `just xtask prerelease`. It computes the affected
packages (directly changed since their last tag, plus dependency-graph propagation), bumps them to
`x.y.z-next.<short-sha>`, and publishes under the `next` channel — without committing
(`cargo publish --allow-dirty`). The set of prereleased packages is reported via the job output
(`prereleased`, `packages`) and the step summary.

This workflow is used to test a package's behavior prior to the official release.

### Artifacts

In a monorepo configuration, it performs the task of merging artifacts generated from multiple packages into a single one or spreading them back to their original locations.

This is used in conjunction with the artifact action in GitHub Actions.

### Attw

Run [attw](https://github.com/arethetypeswrong/arethetypeswrong.github.io) to check npm package is correct before the publishing.
