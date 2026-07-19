import { BridgeError, invoke, platform } from '@wvb/bridge';
import { useEffect, useState } from 'react';
import { version } from '../package.json' with { type: 'json' };
import {
  METHOD_SPECS,
  type MethodParam,
  type Namespace,
  type ResultKind,
  TESTID,
  tid,
} from '../testing/selectors';
import { INVOKERS } from './methods';

type SpecEntry = (typeof METHOD_SPECS)[number];

type CallState =
  | { status: 'idle' }
  | { status: 'pending' }
  | { status: 'ok'; result: unknown }
  | { status: 'error'; error: BridgeError };

const NAMESPACES: readonly Namespace[] = ['source', 'remote', 'updater'];
const THEMES = ['auto', 'light', 'dark'] as const;
type ThemeMode = (typeof THEMES)[number];

/** Sensible starting values so a method can be run without typing. */
function defaultFor(param: MethodParam): string {
  if (param.optional) {
    return '';
  }
  if (param.name === 'bundleName') {
    return 'testbed';
  }
  if (param.name === 'version') {
    return '1.0.0';
  }
  return '';
}

function formatResult(kind: ResultKind, result: unknown): string {
  if (kind === 'void' || result === undefined) {
    return '(void)';
  }
  return JSON.stringify(result, null, 2);
}

function formatError(error: BridgeError): string {
  return error.code != null ? `[${error.code}] ${error.message}` : error.message;
}

function countIn(namespace: Namespace): number {
  return METHOD_SPECS.filter(spec => spec.namespace === namespace).length;
}

/** A `namespace.method()` signature, coloured like source. */
function Signature({ namespace, method }: { namespace?: string; method: string }) {
  return (
    <code className="sig">
      {namespace != null ? (
        <>
          <span className="sig__ns">{namespace}</span>
          <span className="sig__dot">.</span>
        </>
      ) : null}
      <span className="sig__fn">{method}</span>
      <span className="sig__paren">()</span>
    </code>
  );
}

/** The response of a call: the value on success, the error on failure. */
function Response({ status, testId }: { status: CallState; testId: string }) {
  if (status.status !== 'ok' && status.status !== 'error') {
    return null;
  }
  const isError = status.status === 'error';
  return (
    <div className={isError ? 'resp resp--err' : 'resp resp--ok'}>
      <div className="resp__bar">
        <span className="resp__tag">{status.status}</span>
        <span className="resp__kind">{isError ? 'BridgeError' : 'result'}</span>
      </div>
      <pre className="resp__body" data-testid={testId} data-status={status.status}>
        {status.status === 'ok' ? formatResult('value', status.result) : formatError(status.error)}
      </pre>
    </div>
  );
}

function StatusChip({ status, testId }: { status: CallState['status']; testId: string }) {
  return (
    <span className="chip" data-testid={testId} data-status={status}>
      <span className="chip__dot" aria-hidden="true" />
      {status}
    </span>
  );
}

function RunButton({
  testId,
  onRun,
  pending,
}: {
  testId: string;
  onRun: () => void;
  pending: boolean;
}) {
  return (
    <button type="button" className="run" data-testid={testId} onClick={onRun} disabled={pending}>
      <span className="run__glyph" aria-hidden="true" />
      Run
    </button>
  );
}

function MethodCard({ spec }: { spec: SpecEntry }) {
  const [inputs, setInputs] = useState<Record<string, string>>(() =>
    Object.fromEntries(spec.params.map(param => [param.name, defaultFor(param)]))
  );
  const [state, setState] = useState<CallState>({ status: 'idle' });

  async function run() {
    setState({ status: 'pending' });
    try {
      const result = await INVOKERS[spec.id](inputs);
      setState({ status: 'ok', result });
    } catch (error) {
      setState({ status: 'error', error: BridgeError.from(error) });
    }
  }

  return (
    <div className="cell" data-testid={tid.method(spec.id)}>
      <div className="cell__head">
        <Signature namespace={spec.namespace} method={spec.method} />
        <RunButton testId={tid.run(spec.id)} onRun={run} pending={state.status === 'pending'} />
      </div>
      <p className="cell__doc">{spec.summary}</p>
      {spec.params.length > 0 ? (
        <div className="args">
          {spec.params.map((param: MethodParam) => (
            <label key={param.name} className="arg">
              <span className="arg__key">
                {param.name}
                {param.optional ? <i className="arg__opt">?</i> : null}
              </span>
              <input
                className="arg__input"
                data-testid={tid.param(spec.id, param.name)}
                value={inputs[param.name] ?? ''}
                placeholder={param.optional ? 'optional' : 'required'}
                spellCheck={false}
                autoComplete="off"
                onChange={e => setInputs(prev => ({ ...prev, [param.name]: e.target.value }))}
              />
            </label>
          ))}
        </div>
      ) : (
        <div className="args args--empty">no arguments</div>
      )}
      <div className="cell__foot">
        <StatusChip status={state.status} testId={tid.status(spec.id)} />
      </div>
      <Response status={state} testId={tid.result(spec.id)} />
    </div>
  );
}

function RawInvokeCard() {
  const [name, setName] = useState('sourceListBundles');
  const [paramsText, setParamsText] = useState('');
  const [state, setState] = useState<CallState>({ status: 'idle' });

  async function run() {
    setState({ status: 'pending' });
    let params: Record<string, unknown> | undefined;
    const trimmed = paramsText.trim();
    if (trimmed !== '') {
      try {
        params = JSON.parse(trimmed);
      } catch (error) {
        setState({
          status: 'error',
          error: BridgeError.from(`invalid JSON params: ${(error as Error).message}`),
        });
        return;
      }
    }
    try {
      const result = await invoke(name, params);
      setState({ status: 'ok', result });
    } catch (error) {
      setState({ status: 'error', error: BridgeError.from(error) });
    }
  }

  return (
    <div className="cell cell--wide" data-testid="method-invoke">
      <div className="cell__head">
        <Signature method="invoke" />
        <RunButton testId={TESTID.invokeRun} onRun={run} pending={state.status === 'pending'} />
      </div>
      <p className="cell__doc">
        Call any command by name — the escape hatch every namespace wraps.
      </p>
      <div className="args">
        <label className="arg">
          <span className="arg__key">name</span>
          <input
            className="arg__input"
            data-testid={TESTID.invokeName}
            value={name}
            spellCheck={false}
            autoComplete="off"
            onChange={e => setName(e.target.value)}
          />
        </label>
        <label className="arg arg--stack">
          <span className="arg__key">
            params <i className="arg__opt">json</i>
          </span>
          <textarea
            className="arg__input arg__input--area"
            data-testid={TESTID.invokeParams}
            value={paramsText}
            placeholder='{ "bundleName": "testbed" }'
            rows={2}
            spellCheck={false}
            onChange={e => setParamsText(e.target.value)}
          />
        </label>
      </div>
      <div className="cell__foot">
        <StatusChip status={state.status} testId={TESTID.invokeStatus} />
      </div>
      <Response status={state} testId={TESTID.invokeResult} />
    </div>
  );
}

function useTheme(): [ThemeMode, () => void] {
  const [theme, setTheme] = useState<ThemeMode>('auto');
  useEffect(() => {
    const root = document.documentElement;
    if (theme === 'auto') {
      root.removeAttribute('data-theme');
    } else {
      root.setAttribute('data-theme', theme);
    }
  }, [theme]);
  const cycle = () => setTheme(t => THEMES[(THEMES.indexOf(t) + 1) % THEMES.length] ?? 'auto');
  return [theme, cycle];
}

export function App() {
  const detected = platform.type ?? 'none';
  const connected = platform.type != null;
  const [theme, cycleTheme] = useTheme();

  return (
    <div className="app" data-testid={TESTID.appShell} data-platform={detected}>
      <header className="bar">
        <div className="bar__brand">
          <span className="bar__logo">wvb</span>
          <span className="bar__name">bridge testbed</span>
          <span className="bar__ver">v{version}</span>
        </div>
        <div className="bar__tools">
          <span className="conn" data-on={connected}>
            <span className="conn__dot" aria-hidden="true" />
            <span className="conn__label">platform</span>
            <b className="conn__val" data-testid={TESTID.platformType}>
              {detected}
            </b>
          </span>
          <button type="button" className="ghost" onClick={cycleTheme} title="Toggle color theme">
            theme:{theme}
          </button>
        </div>
      </header>

      {!connected ? (
        <div className="notice">
          <span className="notice__badge">offline</span>
          <span className="notice__text">
            No native webview-bundle host detected — every call returns a <code>BridgeError</code>.
            Load the testbed inside Electron or Tauri to talk to the real bridge.
          </span>
        </div>
      ) : null}

      <main className="body">
        {NAMESPACES.map(ns => (
          <section key={ns} className="group">
            <div className="group__head">
              <span className="group__label">{ns}</span>
              <span className="group__count">{countIn(ns)}</span>
            </div>
            <div className="grid">
              {METHOD_SPECS.filter(spec => spec.namespace === ns).map(spec => (
                <MethodCard key={spec.id} spec={spec} />
              ))}
            </div>
          </section>
        ))}
        <section className="group">
          <div className="group__head">
            <span className="group__label">invoke</span>
            <span className="group__count group__count--tag">raw</span>
          </div>
          <div className="grid">
            <RawInvokeCard />
          </div>
        </section>
      </main>
    </div>
  );
}
