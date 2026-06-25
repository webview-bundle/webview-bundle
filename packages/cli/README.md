# @wvb/cli

The `wvb` (alias `webview-bundle`) CLI for building, serving, and installing Webview Bundles.

## `wvb builtin`

Installs **builtin** bundles — downloaded from a remote or packed from local workspaces (per your
`wvb.config.ts`) — into a directory as `manifest.json` + `<name>/<name>_<version>.wvb`, the exact
layout every runtime reads. By default it stages into `./.wvb/builtin/bundles`; the platform presets
below default the output to where each platform expects it.

```sh
wvb builtin                       # stage into ./.wvb/builtin/bundles
wvb builtin --tauri               # Tauri preset (see @wvb/tauri README)
wvb builtin --android ./android/app
wvb builtin --ios ./ios/MyApp/Generated/builtin
```

### Mobile presets

The runtime contract is the same as desktop — a filesystem directory of `manifest.json` +
`<name>/<name>_<version>.wvb`. The catch is how each platform ships and exposes that directory:

- **iOS** app-bundle resources **are** real filesystem paths → point the runtime `builtin_dir`
  straight at the embedded folder. But Xcode's "Copy Bundle Resources" **flattens** folders, so the
  staging dir must be added as a **folder reference** (blue folder), not a group.
- **Android** APK assets are **not** filesystem paths (read via `AssetManager`) → at runtime you must
  **extract** them to a real directory (e.g. `filesDir`) and point `builtin_dir` there.

These presets cover the **build-time staging** half. The **runtime resolution** half (iOS
`Bundle.main` path; Android asset extraction) lives in the native libraries.

#### Android — `wvb builtin --android <module>`

Defaults the output to `<module>/src/main/assets/bundles/builtin`, so AGP merges the bundles into the
APK/AAB assets. Wire it into the build before assets are merged, and keep `.wvb` uncompressed:

```kotlin
// <module>/build.gradle.kts
android {
  androidResources { noCompress += "wvb" }   // don't re-compress already-compressed .wvb
}

androidComponents {
  onVariants { variant ->
    val gen = tasks.register<Exec>("generate${variant.name}WvbAssets") {
      commandLine("wvb", "builtin", "--android", project.projectDir.absolutePath)
    }
    // `merge<Variant>Assets` isn't registered yet inside onVariants, so `tasks.named(...)` would
    // throw — match lazily with configureEach instead.
    val mergeAssets = "merge${variant.name.replaceFirstChar { it.uppercase() }}Assets"
    tasks.matching { it.name == mergeAssets }.configureEach { dependsOn(gen) }
  }
}
```

At runtime, extract `bundles/builtin` from assets to `filesDir` (version-gated, off the main thread)
and pass that absolute path as the source's `builtin_dir`.

#### iOS — `wvb builtin --ios <dir>`

Defaults the output to `<dir>`. Add `<dir>` to your Xcode target as a **folder reference**, and
regenerate it from a **Run Script** build phase placed **above** "Copy Bundle Resources":

```sh
# Run Script phase (above Copy Bundle Resources)
"$PROJECT_DIR/node_modules/.bin/wvb" builtin --ios "$SRCROOT/MyApp/Generated/builtin"
```

At runtime, point the source's `builtin_dir` at the folder via
`Bundle.main.resourceURL` (no extraction needed — it's a real path).

> Add the generated staging directory to `.gitignore` (it's build output).

### Options

`wvb builtin --help` lists all flags. Notable ones: `--channel`, `--include`/`--exclude`,
`--config`, `--out` (override the preset default), and the presets `--tauri` / `--tauri-dir`,
`--android <module>`, `--ios <dir>` (mutually exclusive).
