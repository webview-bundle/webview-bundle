# xtask

A command-line tool for managing and automating tasks in a project.

## Releasing

Releases are made with [prp](https://github.com/seokju-na/prp) (`please-release-please`), not
xtask. Packages, their manifests, assets and publish scripts are configured in
[`.prp/config.toml`](../.prp/config.toml).

1. A maintainer runs `yarn prp prepare` on an up-to-date `main`, with a `GITHUB_TOKEN`
   (e.g. `GITHUB_TOKEN=$(gh auth token) yarn prp prepare`). It shows, per package, the commits
   since its last tag (`<name>/<version>`) and the bump they ask for, and lets you exclude
   changes, skip packages, or choose another bump or an exact version. Dependents follow through
   the dependency graph. It then commits the bumps, changelogs and a release file under
   `.prp/releases/main/` to a release branch and opens (or updates) a pull request.

2. Merging that pull request is the trigger: CI sees the added release file and runs
   `yarn prp release`, which publishes the packages, tags the commit, pushes the tags, and creates
   GitHub releases with the configured assets. Every step skips work that is already done, so a
   partial failure is recoverable by re-running the job.

> [!NOTE]
> npm package will be [staged](https://docs.npmjs.com/staged-publishing) published when releasing to a stable channel.
> Author must approve staged packages via [npmjs.com](https://npmjs.com).

### Snapshots

Running the [`release` workflow](../.github/workflows/release.yaml) manually builds every native
target and runs `yarn prp snapshot`. It publishes every package at its current version plus a
snapshot prerelease (`x.y.z-snapshot-<sha>-<yyyymmdd>`) without committing, under the `snapshot`
npm dist-tag. Internal dependencies are pinned to the exact snapshot versions, so install a
snapshot by its exact version. The packages' assets, including the Android/Swift FFI archives that aren't published to a
registry, are attached to one GitHub prerelease tagged with the snapshot prerelease
(`snapshot-<sha>-<yyyymmdd>`), where `@wvb/deno` looks up its native libraries. Re-running
the workflow on the same commit and day retries only what is missing.

This is used to test a package's behavior prior to the official release.

## Commands

### Native binary size

`xtask size collect` measures the native binaries the builds leave in this checkout (the targets
are listed in [`size.ts`](./size.ts)) and writes a JSON report. `xtask size compare` compares
reports against each package's latest stable GitHub release, and posts the result to the step
summary and a sticky PR comment.

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
