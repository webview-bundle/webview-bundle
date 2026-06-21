export function getWindow<T = Window>(): T {
  return (globalThis || window) as T;
}
