import type { RemoteApi, SourceApi, UpdaterApi } from '../index.js';

type Commands<Namespace extends string, Api> = {
  [K in keyof Api as `${Namespace}.${K & string}`]: Api[K] extends (...args: infer P) => infer R
    ? { params: P; result: Awaited<R> }
    : never;
};

export type BridgeCommandMap = Commands<'source', SourceApi> &
  Commands<'remote', RemoteApi> &
  Commands<'updater', UpdaterApi>;

export type MockInvokeCommand = keyof BridgeCommandMap;

export type MockInvokeHandler<K extends MockInvokeCommand> = (
  ...params: BridgeCommandMap[K]['params']
) => BridgeCommandMap[K]['result'] | Promise<BridgeCommandMap[K]['result']>;
