export function makeError(e: unknown): Error {
  if (e instanceof Error) {
    return e;
  }
  if (typeof e === 'object' && e != null) {
    if ('stack' in e || 'name' in e) {
      return e as Error;
    }
    if (typeof (e as any).message === 'string') {
      return new Error((e as any).message);
    }
  }
  return new Error(String(e));
}
