import { toWebviewBundleError } from './error.js';

type AnyFunction = (...args: any[]) => any;
type AnyConstructor = new (...args: any[]) => any;

function isPromise(value: unknown): value is Promise<unknown> {
  return value instanceof Promise;
}

/** Re-throw whatever `fn` throws — or its returned promise rejects with — as a webview-bundle error. */
export function wrapFunction<F extends AnyFunction>(fn: F): F {
  const wrapped = function (this: unknown, ...args: unknown[]): unknown {
    let result: unknown;
    try {
      result = Reflect.apply(fn, this, args);
    } catch (error) {
      throw toWebviewBundleError(error);
    }
    return isPromise(result)
      ? result.catch((error: unknown) => {
          throw toWebviewBundleError(error);
        })
      : result;
  } as F;
  Object.defineProperty(wrapped, 'name', { value: fn.name, configurable: true });
  return wrapped;
}

/**
 * Wrap a native class so a throwing constructor reports a webview-bundle error. A `Proxy` (rather
 * than a subclass) keeps `prototype` identical to the target's, so instances built by native code
 * still satisfy `instanceof`.
 */
export function wrapClass<T extends AnyConstructor>(klass: T): T {
  return new Proxy(klass, {
    construct(target, args, newTarget) {
      try {
        return Reflect.construct(target, args, newTarget);
      } catch (error) {
        throw toWebviewBundleError(error);
      }
    },
  });
}

function isNativeClass(value: unknown): value is AnyConstructor {
  return (
    typeof value === 'function' &&
    value.prototype != null &&
    // A plain function's prototype only carries `constructor`; a napi class carries its methods.
    Object.getOwnPropertyNames(value.prototype).length > 1
  );
}

// `binding.js` and `binding.cjs` share one set of native classes (both load the same `.node`), so
// importing `index.js` and `index.cjs` in the same process would otherwise patch them twice.
const patched = new WeakSet<object>();

function patchPrototype(klass: AnyConstructor): void {
  const proto = klass.prototype;
  if (patched.has(proto)) {
    return;
  }
  patched.add(proto);
  for (const key of Object.getOwnPropertyNames(proto)) {
    if (key === 'constructor') {
      continue;
    }
    const descriptor = Object.getOwnPropertyDescriptor(proto, key);
    if (descriptor == null || !descriptor.configurable) {
      continue;
    }
    if (typeof descriptor.value === 'function') {
      Object.defineProperty(proto, key, {
        ...descriptor,
        value: wrapFunction(descriptor.value as AnyFunction),
      });
    } else if (typeof descriptor.get === 'function') {
      Object.defineProperty(proto, key, { ...descriptor, get: wrapFunction(descriptor.get) });
    }
  }
}

/**
 * Patch every method of every class the native binding exports.
 */
export function patchBinding(binding: Record<string, unknown>): void {
  for (const value of Object.values(binding)) {
    if (isNativeClass(value)) {
      patchPrototype(value);
    }
  }
}
