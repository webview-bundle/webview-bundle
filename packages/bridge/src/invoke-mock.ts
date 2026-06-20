export const INVOKE_MOCK_KEY = '__wvb_invoke_mock__';

export type InvokeMockFn = (name: string, params?: unknown) => Promise<unknown>;
