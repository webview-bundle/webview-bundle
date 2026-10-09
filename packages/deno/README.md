# @wvb/deno

Deno bindings and desktop integration for WebView Bundle.

```ts
// Core bindings
import { loadLib, Source } from '@wvb/deno';

// Desktop integration
import { webviewBundle } from '@wvb/deno/desktop';
```

The native-library installer is available at `@wvb/deno/install`.

Run `deno task check` or `deno task test` from `packages/deno`. Tests require the
native library built with `cargo build -p wvb-deno --release` from the repository
root.
