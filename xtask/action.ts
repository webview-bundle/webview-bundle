import fs from 'node:fs/promises';
import { EOL } from 'node:os';
import path from 'node:path';
import { diffLines } from 'diff';
import { match } from 'ts-pattern';
import { c } from './console.ts';
import { ROOT_DIR } from './consts.ts';
import { type AssetFile, defaultPorts, type GitHubRelease, type Ports } from './ports.ts';
import { type RegistryType, registryOf } from './registry.ts';

/**
 * Every side effect the pipelines perform, as data. Plans emit actions; {@link runActions} is the
 * single executor, so dry-run, logging, and failure collection behave the same everywhere. The
 * GitHub-touching actions are "ensure"-style (they skip whatever already exists), which is what
 * makes a re-run after a partial failure converge.
 */
export type Action =
  | { type: 'write'; path: string; content: string; prevContent?: string }
  | { type: 'command'; cmd: string; args: string[]; path: string }
  | {
      type: 'publish';
      registry: RegistryType;
      manifest: string;
      version: string;
      cmd: string;
      args: string[];
      path: string;
    }
  | { type: 'createTag'; tag: string }
  | { type: 'pushTags'; refspecs: string[] }
  | {
      type: 'ensureRelease';
      tag: string;
      name: string;
      body?: string;
      prerelease?: boolean;
      targetCommitish?: string;
      /** Refresh the body when the release already exists (used by `prerelease` retries). */
      updateBody?: boolean;
    }
  | { type: 'uploadAssets'; tag: string; assets: AssetFile[] };

export function formatAction(action: Action): string {
  switch (action.type) {
    case 'write':
      return ['write action', c.dim(`  path: ${action.path}`)].join(EOL);
    case 'command':
      return [
        'command action',
        c.dim(`  path: ${action.path}`),
        c.dim(`  cmd: ${action.cmd}`),
        c.dim(`  args: ${action.args.join(' ')}`),
      ].join(EOL);
    case 'publish':
      return [
        'publish action',
        c.dim(`  registry: ${action.registry}`),
        c.dim(`  package: ${action.manifest}@${action.version}`),
        c.dim(`  cmd: ${action.cmd} ${action.args.join(' ')}`),
      ].join(EOL);
    case 'createTag':
      return ['create tag action', c.dim(`  tag: ${action.tag}`)].join(EOL);
    case 'pushTags':
      return ['push tags action', ...action.refspecs.map(ref => c.dim(`  - ${ref}`))].join(EOL);
    case 'ensureRelease':
      return ['github release action', c.dim(`  tag: ${action.tag}`)].join(EOL);
    case 'uploadAssets':
      return [
        'upload assets action',
        c.dim(`  tag: ${action.tag}`),
        ...action.assets.map(asset => c.dim(`  - ${asset.name}`)),
      ].join(EOL);
  }
}

export interface RunActionsContext {
  name?: string;
  dryRun?: boolean;
  failFast?: boolean;
  reject?: boolean;
  ports?: Ports;
}

export type RunActionResult =
  | {
      succeed: true;
      action: Action;
      /** Why the action was a no-op (e.g. the registry rejected the publish as a duplicate). */
      skipped?: string;
      /** Action-specific result (e.g. the GitHub release for `ensureRelease`). */
      data?: unknown;
    }
  | { succeed: false; action: Action; error: Error; output?: string };

interface ResolvedContext {
  name: string;
  dryRun: boolean;
  failFast: boolean;
  reject: boolean;
  ports: Ports;
}

export type RunActionsResult =
  | {
      allSucceed: true;
      items: Array<Extract<RunActionResult, { succeed: true }>>;
      ctx: ResolvedContext;
    }
  | { allSucceed: false; items: RunActionResult[]; ctx: ResolvedContext };

/** Per-run state shared between actions (e.g. `ensureRelease` results reused by `uploadAssets`). */
interface RunState {
  releasesByTag: Map<string, GitHubRelease>;
}

export async function runActions(
  actions: Action[],
  initialCtx: RunActionsContext = {}
): Promise<RunActionsResult> {
  const {
    name = 'root',
    dryRun = false,
    failFast = true,
    reject = true,
    ports = defaultPorts,
  } = initialCtx;
  const ctx: ResolvedContext = { name, dryRun, failFast, reject, ports };
  if (dryRun) {
    dryRunActions(name, actions);
    return {
      allSucceed: true,
      items: actions.map(action => ({ succeed: true, action })),
      ctx,
    };
  }
  const state: RunState = { releasesByTag: new Map() };
  const items: RunActionResult[] = [];
  const rejectFailures = (): never => {
    const failureCount = items.filter(x => !x.succeed).length;
    throw new Error(`${c.error(`[${name}]`)} ${failureCount} action(s) failed`);
  };
  for (const action of actions) {
    const item = await runAction(name, action, ports, state);
    items.push(item);
    if (failFast && !item.succeed) {
      if (reject) {
        rejectFailures();
      }
      return { allSucceed: false, items, ctx };
    }
  }
  const result = {
    allSucceed: items.every(x => x.succeed),
    items,
    ctx,
  } as RunActionsResult;
  if (reject && !result.allSucceed) {
    rejectFailures();
  }
  return result;
}

async function runAction(
  name: string,
  action: Action,
  ports: Ports,
  state: RunState
): Promise<RunActionResult> {
  try {
    return await match(action)
      .with({ type: 'write' }, x => runWriteAction(name, x))
      .with({ type: 'command' }, x => runCommandAction(name, x, ports))
      .with({ type: 'publish' }, x => runPublishAction(name, x, ports))
      .with({ type: 'createTag' }, x => runCreateTagAction(name, x, ports))
      .with({ type: 'pushTags' }, x => runPushTagsAction(name, x, ports))
      .with({ type: 'ensureRelease' }, x => runEnsureReleaseAction(name, x, ports, state))
      .with({ type: 'uploadAssets' }, x => runUploadAssetsAction(name, x, ports, state))
      .exhaustive();
  } catch (e) {
    const error = e as Error;
    console.error(`${c.error(`[${name}]`)} ${action.type} action failed: ${error.message}`);
    return { succeed: false, action, error };
  }
}

async function runWriteAction(
  name: string,
  action: Extract<Action, { type: 'write' }>
): Promise<RunActionResult> {
  console.log(`${c.info(`[${name}]`)} ${formatAction(action)}`);
  try {
    const filepath = path.join(ROOT_DIR, action.path);
    await fs.mkdir(path.dirname(filepath), { recursive: true });
    await fs.writeFile(filepath, action.content, 'utf8');
    if (action.prevContent != null) {
      logDiff(action.prevContent, action.content);
    }
    return { succeed: true, action };
  } catch (e) {
    const error = e as Error;
    console.error(`${c.error(`[${name}]`)} write command failed: ${error.message}`);
    return { succeed: false, action, error };
  }
}

async function runCommandAction(
  name: string,
  action: Extract<Action, { type: 'command' }>,
  ports: Ports
): Promise<RunActionResult> {
  console.log(`${c.info(`[${name}]`)} ${formatAction(action)}`);
  const { exitCode, output } = await ports.proc.run(action.cmd, action.args, {
    cwd: path.join(ROOT_DIR, action.path),
    prefix: `${c.info(`[${name}]`)} `,
  });
  if (exitCode !== 0) {
    console.error(`${c.error(`[${name}]`)} command action failed: exitCode=${exitCode}`);
    return {
      succeed: false,
      action,
      error: new Error(`command failed with exitCode: ${exitCode}`),
      output,
    };
  }
  return { succeed: true, action };
}

async function runPublishAction(
  name: string,
  action: Extract<Action, { type: 'publish' }>,
  ports: Ports
): Promise<RunActionResult> {
  console.log(`${c.info(`[${name}]`)} ${formatAction(action)}`);
  const { exitCode, output } = await ports.proc.run(action.cmd, action.args, {
    cwd: path.join(ROOT_DIR, action.path),
    prefix: `${c.info(`[${name}]`)} `,
  });
  if (exitCode === 0) {
    return { succeed: true, action };
  }
  // A duplicate-version rejection means the version is already published — notably npm's *staged*
  // publishes, which stay invisible to the registry existence check until approved.
  if (registryOf(action.registry).isDuplicateRejection(output)) {
    console.log(
      `${c.warn(`[${name}]`)} ${action.manifest}@${action.version} rejected as a duplicate. treating as already published.`
    );
    return { succeed: true, action, skipped: 'already published' };
  }
  console.error(`${c.error(`[${name}]`)} publish action failed: exitCode=${exitCode}`);
  return {
    succeed: false,
    action,
    error: new Error(`publish failed with exitCode: ${exitCode}`),
    output,
  };
}

function runCreateTagAction(
  name: string,
  action: Extract<Action, { type: 'createTag' }>,
  ports: Ports
): RunActionResult {
  const git = ports.git;
  if (git == null) {
    throw new Error('git effects are unavailable (no repository)');
  }
  const created = git.createTag(action.tag);
  console.log(`${c.success(`[${name}]`)} tag: ${created}`);
  return { succeed: true, action };
}

async function runPushTagsAction(
  name: string,
  action: Extract<Action, { type: 'pushTags' }>,
  ports: Ports
): Promise<RunActionResult> {
  if (ports.github == null || ports.git == null) {
    logWouldRun(name, action);
    return { succeed: true, action, skipped: 'no github token' };
  }
  await ports.git.pushTags(action.refspecs);
  console.log(`${c.success(`[${name}]`)} pushed ${action.refspecs.length} tag(s)`);
  return { succeed: true, action };
}

async function runEnsureReleaseAction(
  name: string,
  action: Extract<Action, { type: 'ensureRelease' }>,
  ports: Ports,
  state: RunState
): Promise<RunActionResult> {
  const github = ports.github;
  if (github == null) {
    logWouldRun(name, action);
    return { succeed: true, action, skipped: 'no github token' };
  }
  let release = await github.findReleaseByTag(action.tag);
  if (release != null) {
    console.log(`${c.warn(`[${name}]`)} github release already exists: ${action.tag}`);
    if (action.updateBody && action.body != null) {
      await github.updateReleaseBody(release.id, action.body);
    }
  } else {
    release = await github.createRelease({
      tag: action.tag,
      name: action.name,
      body: action.body,
      prerelease: action.prerelease,
      targetCommitish: action.targetCommitish,
    });
    console.log(`${c.success(`[${name}]`)} github release: ${action.tag}`);
  }
  state.releasesByTag.set(action.tag, release);
  return { succeed: true, action, data: release };
}

/**
 * Reconcile a release's assets: an asset that is already fully uploaded is skipped, and a stub
 * left by an interrupted upload (which would block re-uploading under the same name) is deleted
 * first. Returns the asset names now on the release as `data`.
 */
async function runUploadAssetsAction(
  name: string,
  action: Extract<Action, { type: 'uploadAssets' }>,
  ports: Ports,
  state: RunState
): Promise<RunActionResult> {
  const github = ports.github;
  if (github == null) {
    logWouldRun(name, action);
    return { succeed: true, action, skipped: 'no github token', data: [] };
  }
  if (action.assets.length === 0) {
    return { succeed: true, action, data: [] };
  }
  const release =
    state.releasesByTag.get(action.tag) ?? (await github.findReleaseByTag(action.tag));
  if (release == null) {
    throw new Error(`github release not found for tag: ${action.tag}`);
  }
  const existing = await github.listReleaseAssets(release.id);
  const uploaded: string[] = [];
  for (const asset of action.assets) {
    const prior = existing.find(x => x.name === asset.name);
    if (prior != null) {
      if (prior.state === 'uploaded') {
        console.log(`  ${c.dim(`asset already uploaded: ${asset.name}`)}`);
        uploaded.push(asset.name);
        continue;
      }
      await github.deleteReleaseAsset(prior.id);
    }
    await github.uploadReleaseAsset(release.id, asset);
    console.log(`  ${c.dim(`asset: ${asset.name}`)}`);
    uploaded.push(asset.name);
  }
  return { succeed: true, action, data: uploaded };
}

function logWouldRun(name: string, action: Action) {
  console.log(`${c.info(`[${name}]`)} will run: ${formatAction(action)}`);
}

function dryRunActions(name: string, actions: Action[]) {
  for (const action of actions) {
    console.log(`${c.info(`[${name}]`)} ${formatAction(action)}`);
    if (action.type === 'write' && action.prevContent != null) {
      logDiff(action.prevContent, action.content);
    }
  }
}

function logDiff(prevContent: string, content: string) {
  const diff = diffLines(prevContent, content);
  let modified: number | undefined;
  let lineNo = 0;
  for (const change of diff) {
    if (!change.added && !change.removed) {
      if (modified != null) {
        lineNo += modified;
        modified = undefined;
      }
      lineNo += change.count ?? 0;
      continue;
    }
    let changeLineNo = lineNo;
    const lines = change.value.trimEnd().split('\n');
    for (const line of lines) {
      changeLineNo += 1;
      const lineStr = String(changeLineNo).padStart(3, ' ');
      const color = (str: string | number): string => {
        if (change.added) {
          return c.success(str);
        }
        return c.error(str);
      };
      const diffPrefix = change.added ? '+' : '-';
      console.log(`  ${c.dim(lineStr)}|${diffPrefix}${color(line)}`);
    }
    modified = change.count;
  }
}
