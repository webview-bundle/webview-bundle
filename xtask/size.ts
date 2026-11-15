import fs from 'node:fs/promises';
import path from 'node:path';
import semver from 'semver';
import { glob } from 'tinyglobby';
import { GITHUB_REPO, ROOT_DIR } from './consts.ts';
import type { GitHubClient } from './github.ts';
import type { Package } from './package.ts';

/**
 * Native binary sizes, per package, keyed by the file's path relative to the package's artifact
 * directory (which is also its GitHub release asset name). Binaries shipped inside a zip asset are
 * keyed as `<zip>/<entry path>` with their uncompressed size, so each ABI/slice is tracked on its own.
 */
export interface SizeReport {
  version: 1;
  packages: Record<string, Record<string, number>>;
}

/** The release asset each package's {@link SizeReport} slice is uploaded as. */
export const SIZE_REPORT_ASSET = 'sizes.json';

const BINARY_EXTENSIONS = new Set(['.node', '.so', '.dylib', '.dll', '.a']);

export function isBinary(file: string): boolean {
  return BINARY_EXTENSIONS.has(path.extname(file).toLowerCase());
}

export function emptyReport(): SizeReport {
  return { version: 1, packages: {} };
}

export interface ZipEntry {
  name: string;
  size: number;
}

/** List a zip's entries (uncompressed sizes) from its central directory. Zip64 is not supported. */
export function readZipEntries(buf: Buffer): ZipEntry[] {
  const EOCD_SIG = 0x06054b50;
  const CD_SIG = 0x02014b50;
  // The end-of-central-directory record is 22 bytes plus a comment of up to 65535 bytes.
  const minOffset = Math.max(0, buf.length - 22 - 0xffff);
  let eocd = -1;
  for (let i = buf.length - 22; i >= minOffset; i -= 1) {
    if (buf.readUInt32LE(i) === EOCD_SIG) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) {
    throw new Error('invalid zip: end of central directory not found');
  }
  const count = buf.readUInt16LE(eocd + 10);
  let offset = buf.readUInt32LE(eocd + 16);
  const entries: ZipEntry[] = [];
  for (let i = 0; i < count; i += 1) {
    if (buf.readUInt32LE(offset) !== CD_SIG) {
      throw new Error('invalid zip: bad central directory entry');
    }
    const size = buf.readUInt32LE(offset + 24);
    const nameLen = buf.readUInt16LE(offset + 28);
    const extraLen = buf.readUInt16LE(offset + 30);
    const commentLen = buf.readUInt16LE(offset + 32);
    const name = buf.toString('utf8', offset + 46, offset + 46 + nameLen);
    if (!name.endsWith('/')) {
      entries.push({ name, size });
    }
    offset += 46 + nameLen + extraLen + commentLen;
  }
  return entries;
}

/** The binary entries of a zip, keyed as `<zipName>/<entry>`. */
export function zipBinarySizes(zipName: string, buf: Buffer): Record<string, number> {
  const sizes: Record<string, number> = {};
  for (const entry of readZipEntries(buf)) {
    if (isBinary(entry.name)) {
      sizes[`${zipName}/${entry.name}`] = entry.size;
    }
  }
  return sizes;
}

async function measureFile(key: string, absolutePath: string): Promise<Record<string, number>> {
  if (key.toLowerCase().endsWith('.zip')) {
    return zipBinarySizes(key, await fs.readFile(absolutePath));
  }
  if (isBinary(key)) {
    return { [key]: (await fs.stat(absolutePath)).size };
  }
  return {};
}

/**
 * Measure the native binaries of every package with `artifacts` configured.
 *
 * Without `mergeDir`, files are read where builds leave them (each artifact's `src` + `patterns`);
 * with it, from the `xtask artifacts merge` layout (`<mergeDir>/<package path>/**`). Both yield the
 * same keys, so reports from CI builds and from the release job are comparable.
 */
export async function collectSizes(packages: Package[], mergeDir?: string): Promise<SizeReport> {
  const report = emptyReport();
  for (const pkg of packages) {
    if (pkg.artifacts.length === 0) {
      continue;
    }
    const sizes: Record<string, number> = {};
    const sources =
      mergeDir != null
        ? [{ dir: path.resolve(ROOT_DIR, mergeDir, pkg.path), patterns: ['**/*'] }]
        : pkg.artifacts.map(a => ({
            dir: path.join(ROOT_DIR, pkg.path, a.src),
            patterns: a.patterns,
          }));
    for (const source of sources) {
      const files = await glob(source.patterns, { cwd: source.dir, onlyFiles: true });
      for (const file of files.sort()) {
        Object.assign(sizes, await measureFile(file, path.join(source.dir, file)));
      }
    }
    if (Object.keys(sizes).length > 0) {
      report.packages[pkg.name] = sizes;
    }
  }
  return report;
}

/** Merge reports (e.g. one per CI matrix job); later reports win on duplicate keys. */
export function mergeReports(reports: SizeReport[]): SizeReport {
  const merged = emptyReport();
  for (const report of reports) {
    for (const [pkg, sizes] of Object.entries(report.packages)) {
      merged.packages[pkg] = { ...merged.packages[pkg], ...sizes };
    }
  }
  return merged;
}

export function parseReport(text: string): SizeReport {
  const data = JSON.parse(text) as Partial<SizeReport>;
  if (data.version !== 1 || data.packages == null || typeof data.packages !== 'object') {
    throw new Error('unsupported size report');
  }
  return data as SizeReport;
}

/** The report narrowed to one package (what gets uploaded with that package's release). */
export function sliceReport(report: SizeReport, pkgName: string): SizeReport | null {
  const sizes = report.packages[pkgName];
  if (sizes == null || Object.keys(sizes).length === 0) {
    return null;
  }
  return { version: 1, packages: { [pkgName]: sizes } };
}

export interface Baseline {
  tag: string;
  sizes: Record<string, number>;
}

/** The latest stable GitHub release tag of a package (`<name>/<semver>`), or `null`. */
export async function findLatestReleaseTag(
  client: GitHubClient,
  pkgName: string
): Promise<{ id: number; tag: string } | null> {
  const releases = await client.paginate(client.rest.repos.listReleases, {
    owner: GITHUB_REPO.owner,
    repo: GITHUB_REPO.name,
    per_page: 100,
  });
  const prefix = `${pkgName}/`;
  let latest: { id: number; tag: string; version: string } | null = null;
  for (const release of releases) {
    if (release.draft || release.prerelease || !release.tag_name.startsWith(prefix)) {
      continue;
    }
    const version = semver.valid(release.tag_name.slice(prefix.length));
    if (version == null || semver.prerelease(version) != null) {
      continue;
    }
    if (latest == null || semver.gt(version, latest.version)) {
      latest = { id: release.id, tag: release.tag_name, version };
    }
  }
  return latest;
}

async function downloadAsset(client: GitHubClient, assetId: number): Promise<Buffer> {
  const res = await client.rest.repos.getReleaseAsset({
    owner: GITHUB_REPO.owner,
    repo: GITHUB_REPO.name,
    asset_id: assetId,
    headers: { accept: 'application/octet-stream' },
  });
  return Buffer.from(res.data as unknown as ArrayBuffer);
}

/**
 * The sizes of a package's latest stable release. Prefers its `sizes.json` asset; releases made
 * before that asset existed fall back to the asset list (binary sizes come with the listing, zips
 * are downloaded to read their central directory).
 */
export async function fetchBaseline(
  client: GitHubClient,
  pkgName: string
): Promise<Baseline | null> {
  const release = await findLatestReleaseTag(client, pkgName);
  if (release == null) {
    return null;
  }
  const assets = await client.paginate(client.rest.repos.listReleaseAssets, {
    owner: GITHUB_REPO.owner,
    repo: GITHUB_REPO.name,
    release_id: release.id,
    per_page: 100,
  });
  const reportAsset = assets.find(a => a.name === SIZE_REPORT_ASSET);
  if (reportAsset != null) {
    const report = parseReport((await downloadAsset(client, reportAsset.id)).toString('utf8'));
    return { tag: release.tag, sizes: report.packages[pkgName] ?? {} };
  }
  const sizes: Record<string, number> = {};
  for (const asset of assets) {
    if (asset.name.toLowerCase().endsWith('.zip')) {
      Object.assign(sizes, zipBinarySizes(asset.name, await downloadAsset(client, asset.id)));
    } else if (isBinary(asset.name)) {
      sizes[asset.name] = asset.size;
    }
  }
  return { tag: release.tag, sizes };
}

export interface SizeRow {
  name: string;
  base: number | null;
  current: number;
}

export interface PackageComparison {
  pkg: string;
  baseTag: string | null;
  rows: SizeRow[];
}

export function compareSizes(
  current: SizeReport,
  baselines: Map<string, Baseline | null>
): PackageComparison[] {
  return Object.keys(current.packages)
    .sort()
    .map(pkg => {
      const baseline = baselines.get(pkg) ?? null;
      const rows = Object.entries(current.packages[pkg]!)
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([name, size]) => ({ name, base: baseline?.sizes[name] ?? null, current: size }));
      return { pkg, baseTag: baseline?.tag ?? null, rows };
    });
}

export function formatBytes(bytes: number): string {
  const abs = Math.abs(bytes);
  if (abs < 1024) {
    return `${bytes} B`;
  }
  if (abs < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KiB`;
  }
  return `${(bytes / 1024 / 1024).toFixed(2)} MiB`;
}

function formatDelta(row: SizeRow, threshold: number): { delta: string; percent: string } {
  if (row.base == null) {
    return { delta: 'new', percent: '' };
  }
  const diff = row.current - row.base;
  if (diff === 0) {
    return { delta: '0', percent: '0.00%' };
  }
  const ratio = row.base === 0 ? Number.POSITIVE_INFINITY : (diff / row.base) * 100;
  const sign = diff > 0 ? '+' : '-';
  // Arrows only for moves of 1% or more; smaller deltas are mostly link-order noise.
  const icon = ratio >= threshold ? ' ⚠️' : Math.abs(ratio) < 1 ? '' : diff < 0 ? ' 🔻' : ' 🔺';
  return {
    delta: `${sign}${formatBytes(Math.abs(diff))}`,
    percent: `${sign}${Math.abs(ratio).toFixed(2)}%${icon}`,
  };
}

export function reportMarker(id: string): string {
  return `<!-- wvb-size-report:${id} -->`;
}

export interface RenderOptions {
  id: string;
  title: string;
  /** Percent increase at which a row is flagged with ⚠️. */
  threshold: number;
  commit?: string;
}

/** Render a comparison as a Markdown PR comment / step summary. */
export function renderComparison(comparisons: PackageComparison[], opts: RenderOptions): string {
  const lines: string[] = [reportMarker(opts.id), `## ${opts.title}`, ''];
  if (comparisons.length === 0) {
    lines.push('No native binaries were measured.');
    return `${lines.join('\n')}\n`;
  }
  const flagged = comparisons.flatMap(c =>
    c.rows.filter(
      r => r.base != null && r.base > 0 && ((r.current - r.base) / r.base) * 100 >= opts.threshold
    )
  );
  const against = comparisons.map(c =>
    c.baseTag != null ? `\`${c.baseTag}\`` : `\`${c.pkg}\` (no release)`
  );
  lines.push(
    `Compared against the latest releases: ${against.join(', ')}.${opts.commit != null ? ` Measured at ${opts.commit}.` : ''}`,
    ''
  );
  if (flagged.length > 0) {
    lines.push(
      `> [!WARNING]`,
      `> ${flagged.length} binar${flagged.length === 1 ? 'y' : 'ies'} grew by ${opts.threshold}% or more.`,
      ''
    );
  }
  for (const c of comparisons) {
    const base = c.rows.reduce((sum, r) => sum + (r.base ?? 0), 0);
    const current = c.rows.reduce((sum, r) => sum + (r.base != null ? r.current : 0), 0);
    const total = base > 0 ? formatDelta({ name: '', base, current }, opts.threshold) : null;
    const summary = total != null ? ` — ${total.delta} (${total.percent.trim()})` : '';
    lines.push(
      '<details>',
      `<summary><b>${c.pkg}</b>${summary}</summary>`,
      '',
      '| Binary | Base | Current | Δ | Δ% |',
      '| --- | ---: | ---: | ---: | ---: |'
    );
    for (const row of c.rows) {
      const { delta, percent } = formatDelta(row, opts.threshold);
      const baseText = row.base != null ? formatBytes(row.base) : '-';
      lines.push(
        `| \`${row.name}\` | ${baseText} | ${formatBytes(row.current)} | ${delta} | ${percent} |`
      );
    }
    lines.push('', '</details>', '');
  }
  return `${lines.join('\n')}\n`;
}

/** Create or update the PR comment carrying `marker`. */
export async function upsertPullRequestComment(
  client: GitHubClient,
  pr: number,
  marker: string,
  body: string
): Promise<string> {
  const repo = { owner: GITHUB_REPO.owner, repo: GITHUB_REPO.name };
  const comments = await client.paginate(client.rest.issues.listComments, {
    ...repo,
    issue_number: pr,
    per_page: 100,
  });
  const existing = comments.find(comment => comment.body?.includes(marker));
  if (existing != null) {
    const res = await client.rest.issues.updateComment({ ...repo, comment_id: existing.id, body });
    return res.data.html_url;
  }
  const res = await client.rest.issues.createComment({ ...repo, issue_number: pr, body });
  return res.data.html_url;
}
