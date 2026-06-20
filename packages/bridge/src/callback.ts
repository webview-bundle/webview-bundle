import { getWindow } from './window.js';

interface CallbackBagWindow {
  __wvb_bridge_callbacks__: Record<string, (...args: any) => any>;
}

export class CallbackBag {
  private ids: string[] = [];

  generate(callback: (...args: any) => any): string {
    const w = getWindow<CallbackBagWindow>();
    w.__wvb_bridge_callbacks__ ??= {};

    const id = `cb_${generateId()}`;
    w.__wvb_bridge_callbacks__[id] = callback;
    this.ids.push(id);

    return `(
    function() {
      var cb = __wvb_bridge_callbacks__ != null
        ? __wvb_bridge_callbacks__["${id}"]
        : undefined;
      if (cb == null) {
        throw new Error("cannot find callback: ${id}");
      }
      return cb.apply(undefined, arguments);
    }
    )`.replace(/(\n|\s{2,})/g, ' ');
  }

  clean(): void {
    const w = getWindow<CallbackBagWindow>();
    for (const id of this.ids) {
      if (w.__wvb_bridge_callbacks__ != null) {
        Reflect.deleteProperty(w.__wvb_bridge_callbacks__, id);
      }
    }
    this.ids = [];
  }
}

function generateId(): string {
  const w = getWindow();
  if (typeof w?.crypto?.randomUUID === 'function') {
    return w.crypto.randomUUID().replace(/-/g, '');
  }
  // Polyfill for environments without crypto.randomUUID: build an RFC 4122
  // version 4 UUID, returned as 32 hex chars to match the stripped form above.
  const bytes = new Uint8Array(16);
  if (typeof w?.crypto?.getRandomValues === 'function') {
    w.crypto.getRandomValues(bytes);
  } else {
    for (let i = 0; i < bytes.length; i++) {
      bytes[i] = Math.floor(Math.random() * 256);
    }
  }
  // Set the version (4) and variant (10xx) bits.
  bytes[6] = ((bytes[6] ?? 0) & 0x0f) | 0x40;
  bytes[8] = ((bytes[8] ?? 0) & 0x3f) | 0x80;
  let id = '';
  for (const byte of bytes) {
    id += byte.toString(16).padStart(2, '0');
  }
  return id;
}
