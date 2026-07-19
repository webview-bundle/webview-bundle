import { BridgeError, invoke, platform } from '@wvb/bridge';
import { useState } from 'react';
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

/** The terminal output of a call: the value on success, the error on failure. */
function Output({ status, testId }: { status: CallState; testId: string }) {
  if (status.status !== 'ok' && status.status !== 'error') {
    return null;
  }
  const isError = status.status === 'error';
  return (
    <pre
      className={isError ? 'output output--error' : 'output'}
      data-testid={testId}
      data-status={status.status}
    >
      {status.status === 'ok' ? formatResult('value', status.result) : formatError(status.error)}
    </pre>
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
    <div className="card" data-testid={tid.method(spec.id)}>
      <div className="card__head">
        <code className="card__id">{spec.id}</code>
        <button
          type="button"
          className="card__run"
          data-testid={tid.run(spec.id)}
          onClick={run}
          disabled={state.status === 'pending'}
        >
          Run
        </button>
      </div>
      <p className="card__summary">{spec.summary}</p>
      {spec.params.length > 0 ? (
        <div className="card__params">
          {spec.params.map((param: MethodParam) => (
            <label key={param.name} className="field">
              <span className="field__name">
                {param.name}
                {param.optional ? <span className="field__opt">?</span> : null}
              </span>
              <input
                className="field__input"
                data-testid={tid.param(spec.id, param.name)}
                value={inputs[param.name] ?? ''}
                placeholder={param.optional ? 'optional' : ''}
                onChange={e => setInputs(prev => ({ ...prev, [param.name]: e.target.value }))}
              />
            </label>
          ))}
        </div>
      ) : null}
      <div className="card__foot">
        <span className="status" data-testid={tid.status(spec.id)} data-status={state.status}>
          {state.status}
        </span>
      </div>
      <Output status={state} testId={tid.result(spec.id)} />
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
    <div className="card" data-testid="method-invoke">
      <div className="card__head">
        <code className="card__id">invoke</code>
        <button
          type="button"
          className="card__run"
          data-testid={TESTID.invokeRun}
          onClick={run}
          disabled={state.status === 'pending'}
        >
          Run
        </button>
      </div>
      <p className="card__summary">
        Call any command by name — the escape hatch every namespace wraps.
      </p>
      <div className="card__params">
        <label className="field">
          <span className="field__name">name</span>
          <input
            className="field__input"
            data-testid={TESTID.invokeName}
            value={name}
            onChange={e => setName(e.target.value)}
          />
        </label>
        <label className="field">
          <span className="field__name">
            params <span className="field__opt">json</span>
          </span>
          <textarea
            className="field__input field__input--area"
            data-testid={TESTID.invokeParams}
            value={paramsText}
            placeholder='{ "bundleName": "testbed" }'
            rows={2}
            onChange={e => setParamsText(e.target.value)}
          />
        </label>
      </div>
      <div className="card__foot">
        <span className="status" data-testid={TESTID.invokeStatus} data-status={state.status}>
          {state.status}
        </span>
      </div>
      <Output status={state} testId={TESTID.invokeResult} />
    </div>
  );
}

export function App() {
  const detected = platform.type ?? 'none';
  const showNoHost = platform.type == null;

  return (
    <div className="app" data-testid={TESTID.appShell} data-platform={detected}>
      <header className="app__header">
        <div className="app__title">
          <h1>Bridge Testbed</h1>
          <span className="app__version">v{version}</span>
        </div>
        <div className="app__meta">
          <span className="pill">
            platform: <b data-testid={TESTID.platformType}>{detected}</b>
          </span>
        </div>
      </header>
      {showNoHost ? (
        <p className="banner">
          No native webview-bundle host detected — bridge calls will fail here. Open the testbed
          inside a native host (Electron, Tauri, …) to exercise the methods.
        </p>
      ) : null}
      <main className="app__main">
        {NAMESPACES.map(ns => (
          <section key={ns} className="ns">
            <h2 className="ns__title">{ns}</h2>
            <div className="grid">
              {METHOD_SPECS.filter(spec => spec.namespace === ns).map(spec => (
                <MethodCard key={spec.id} spec={spec} />
              ))}
            </div>
          </section>
        ))}
        <section className="ns">
          <h2 className="ns__title">
            invoke <span className="ns__sub">low-level</span>
          </h2>
          <div className="grid">
            <RawInvokeCard />
          </div>
        </section>
      </main>
    </div>
  );
}
