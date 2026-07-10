# xtask

A command-line tool for managing and automating tasks in a project.

## Configuration

Packages are declared in the root [`xtask.config.ts`](../xtask.config.ts), loaded with a native
`import()` and validated strictly (unknown fields are an error). Each `packages` entry is either a
glob of package directories (released with the default config) or an object with a `path` plus
per-package config (`artifacts`, `assets`, `beforePublishScripts`, …) — the object form wins when
both match the same directory. A directory counts as a package only when it directly holds a
`package.json`/`Cargo.toml`/`deno.json`; manifests below it (e.g. napi platform packages under
`packages/node/npm/*`) are versioned as part of that package.

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
   publishes every package bumped by the merge commit, tags the merge commit, pushes the tags, and
   creates GitHub releases (uploading each package's configured assets). Every step skips work
   that is already done, so a partial failure is recoverable by re-running the job.

> [!NOTE]
> npm package will be [staged](https://docs.npmjs.com/staged-publishing) published when releasing to a stable channel.
> Author must approve staged packages via [npmjs.com](https://npmjs.com).

### Pre-Releasing

On every other base-branch commit, CI runs `just xtask prerelease`. It computes the affected
packages (directly changed since their last tag, plus dependency-graph propagation), bumps them to
`x.y.z-next.<short-sha>`, and publishes under the `next` channel — without committing
(`cargo publish --allow-dirty`). Each target's publish status (including failures) is reported
via the job output (`prereleased`, `packages`) and the step summary. Re-running the job on the
same commit retries only what is missing.

This workflow is used to test a package's behavior prior to the official release.

### Artifacts

In a monorepo configuration, it performs the task of merging artifacts generated from multiple packages into a single one or spreading them back to their original locations.

This is used in conjunction with the artifact action in GitHub Actions.

### Attw

Run [attw](https://github.com/arethetypeswrong/arethetypeswrong.github.io) to check npm package is correct before the publishing.

### Resolving lockfile conflicts

When several PRs are open at once, rebasing onto the base branch routinely produces a `yarn.lock`
conflict even though every `package.json` merges cleanly. `xtask resolve-lockfile --pr <number>`
fixes that mechanically: it merges the base branch into the PR head, and **only if `yarn.lock` is
the sole conflict**, takes the base branch's lockfile, runs `yarn install` to regenerate it against
the merged `package.json` set, commits the merge, and pushes it back to the PR branch. Any other
conflict aborts and asks for a human; the bot never merges the PR itself.

The [`resolve-lockfile` workflow](../.github/workflows/resolve-lockfile.yaml) drives it: a maintainer
opens a PR comment **starting with** `/merge-lockfile` or `/resolve-lockfile` (either works). As its
first step — before the slow checkout and toolchain setup — the workflow reacts 👀 to the comment so
the commenter gets immediate feedback that the command was accepted; the resolver later adds 🚀 /
😕 / 👎 to report the outcome. The workflow pre-filters on `author_association`, but the command then
verifies the commenter's *actual* write access via the API (`author_association` alone includes
read-only org members). Only same-repository branches are supported — forks are out of scope because
the default token cannot push to a fork.

Preconditions checked before acting: the commenter has write access, the PR is open, it is not from
a fork, its head's status checks are green, and **it does not modify the Yarn toolchain**
(`.yarnrc.yml` / `.yarn/releases` / `.yarn/plugins`) — the bot refuses those because `yarn install`
would otherwise execute a PR-supplied Yarn binary or plugin under the bot token. `--skip-checks`
overrides the status-check gate for local runs; `--dry-run` resolves without pushing.
