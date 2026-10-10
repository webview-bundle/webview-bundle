import { type BundleSourceVersion, type ListBundleItem, platform, source } from '@wvb/bridge';
import './style.css';

const BUNDLE_NAME = '{{bundleName}}';

type Serving =
  | { kind: 'bundle'; version: BundleSourceVersion }
  | { kind: 'unloaded' }
  | { kind: 'browser' }
  | { kind: 'error'; detail: string };

interface Diagnostics {
  serving: Serving;
  bundles: ListBundleItem[];
}

async function readDiagnostics(): Promise<Diagnostics> {
  if (platform.type == null) {
    return { serving: { kind: 'browser' }, bundles: [] };
  }
  try {
    const version = await source.loadVersion(BUNDLE_NAME);
    const bundles = await source.listBundles().catch(() => [] as ListBundleItem[]);
    return {
      serving: version == null ? { kind: 'unloaded' } : { kind: 'bundle', version },
      bundles,
    };
  } catch (e) {
    return { serving: { kind: 'error', detail: messageOf(e) }, bundles: [] };
  }
}

function messageOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className != null) {
    node.className = className;
  }
  if (text != null) {
    node.textContent = text;
  }
  return node;
}

interface Status {
  label: string;
  detail: string;
  tone: 'ok' | 'warn' | 'idle' | 'err';
}

function statusOf(serving: Serving): Status {
  switch (serving.kind) {
    case 'bundle':
      return {
        label: 'Served from a .wvb bundle',
        detail: `The ${platform.type} host is serving this page out of the ${serving.version.type} bundle it has loaded for "${BUNDLE_NAME}".`,
        tone: 'ok',
      };
    case 'unloaded':
      return {
        label: 'No bundle loaded',
        detail: `The ${platform.type} host is reachable but has no bundle named "${BUNDLE_NAME}" loaded — which is what you see when it proxies the webview to your dev server instead of serving a packed bundle.`,
        tone: 'warn',
      };
    case 'browser':
      return {
        label: 'Browser preview',
        detail:
          'No native host is attached, so the bridge has nothing to answer with. Load this page inside your app to see which bundle is serving it.',
        tone: 'idle',
      };
    case 'error':
      return {
        label: 'Bridge error',
        detail: serving.detail,
        tone: 'err',
      };
  }
}

/** `location.origin` is the string "null" for opaque origins some custom schemes produce. */
function originLabel(): string {
  const { origin, protocol, host } = window.location;
  return origin !== '' && origin !== 'null' ? origin : `${protocol}//${host}`;
}

function renderFacts(serving: Serving): HTMLElement {
  const version = serving.kind === 'bundle' ? serving.version : null;
  const facts: Array<[string, string, boolean]> = [
    ['Bundle', BUNDLE_NAME, false],
    ['Version', version?.version ?? 'unknown', version == null],
    ['Source', version?.type ?? 'none', version == null],
    ['Host', platform.type ?? 'browser', platform.type == null],
    ['Origin', originLabel(), false],
    ['Build', import.meta.env.DEV ? 'vite dev' : 'vite build', false],
  ];

  const list = el('dl', 'facts');
  for (const [key, value, dim] of facts) {
    const row = el('div', 'facts__row');
    row.append(
      el('dt', 'facts__key', key),
      el('dd', dim ? 'facts__val facts__val--dim' : 'facts__val', value)
    );
    list.append(row);
  }
  return list;
}

function renderBundles(bundles: ListBundleItem[]): HTMLElement | null {
  if (bundles.length === 0) {
    return null;
  }
  const section = el('section', 'bundles');
  section.append(el('h2', 'bundles__title', `Installed bundles (${bundles.length})`));

  const list = el('ul', 'bundles__list');
  for (const { type, item } of bundles) {
    const row = el('li', item.current ? 'bundle bundle--current' : 'bundle');
    row.append(
      el('span', 'bundle__name', item.name),
      el('span', 'bundle__version', item.version),
      el('span', 'bundle__type', item.current ? `${type} · current` : type)
    );
    list.append(row);
  }
  section.append(list);
  return section;
}

function render(root: HTMLElement, diagnostics: Diagnostics, onRefresh: () => void): void {
  const { serving, bundles } = diagnostics;
  const status = statusOf(serving);

  const header = el('header', 'card__header');
  const title = el('div', 'title');
  title.append(
    el('h1', 'title__name', BUNDLE_NAME),
    el('p', 'title__sub', 'A Webview Bundle starter')
  );
  header.append(el('div', 'mark', 'wvb'), title);

  const statusSection = el('section', 'status');
  const badge = el('p', `badge badge--${status.tone}`);
  badge.append(el('span', 'badge__dot'), el('span', undefined, status.label));
  statusSection.append(badge, el('p', 'status__detail', status.detail));

  const footer = el('footer', 'card__footer');
  const hint = el('p', 'hint');
  hint.append(document.createTextNode('Edit '), el('code', undefined, 'src/main.ts'));
  const refresh = el('button', 'button', 'Refresh');
  refresh.type = 'button';
  refresh.addEventListener('click', onRefresh);
  footer.append(hint, refresh);

  const card = el('div', 'card');
  card.append(header, statusSection, renderFacts(serving));
  const bundleList = renderBundles(bundles);
  if (bundleList != null) {
    card.append(bundleList);
  }
  card.append(footer);

  root.replaceChildren(card);
}

async function main(): Promise<void> {
  const root = document.querySelector<HTMLElement>('#app');
  if (root == null) {
    throw new Error('missing #app root element');
  }
  const refresh = (): void => {
    void main();
  };
  render(root, await readDiagnostics(), refresh);
}

void main();
