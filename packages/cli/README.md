# @wvb/cli

The `wvb` (alias `webview-bundle`) CLI for building, serving, and installing Webview Bundles.

## `wvb builtin`

Installs **builtin** bundles — downloaded from a remote or packed from local workspaces (per your
`wvb.config.ts`) — into a directory as `manifest.json` + `<name>/<name>_<version>.wvb`, the exact
layout every runtime reads. By default it stages into `./.wvb/builtin/bundles`; the platform presets
below default the output to where each platform expects it.

```sh
wvb builtin            # stage into ./.wvb/builtin/bundles
wvb builtin --tauri    # Tauri preset — auto-detects src-tauri (see @wvb/tauri README)
wvb builtin --android  # Android preset — auto-detects the app module
wvb builtin --ios      # iOS preset — auto-detects the Xcode/Tuist project
```

Each preset auto-detects its project (like `--tauri`); to point at it explicitly pass the path on the
same flag — `--android=<module>` / `--ios=<project>` (or `--tauri-dir` for Tauri) — or use `--out` to
override the final staging directory.

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

#### Android — `wvb builtin --android`

Auto-detects the application module (the one with `src/main/assets`, via the `com.android.application`
plugin) — across native, React Native, Capacitor, Flutter, and Tauri-mobile (`src-tauri/gen/android`)
layouts — and defaults the output to `<module>/src/main/assets/bundles/builtin`, so AGP merges the
bundles into the APK/AAB assets. Pass `--android=<module>` to point at it explicitly (e.g. for a
multi-app project). Wire it into the build before assets are merged, and keep `.wvb` uncompressed:

```kotlin
// <module>/build.gradle.kts
android {
  androidResources { noCompress += "wvb" }   // don't re-compress already-compressed .wvb
}

androidComponents {
  onVariants { variant ->
    val gen = tasks.register<Exec>("generate${variant.name}WvbAssets") {
      commandLine("wvb", "builtin", "--android=${project.projectDir.absolutePath}")
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

#### iOS — `wvb builtin --ios`

Auto-detects the iOS project — Tuist `Project.swift` first (its generated `*.xcodeproj` is usually
gitignored), else `*.xcworkspace` / `*.xcodeproj` — searching `ios/`, `apple/ios/`, and
`src-tauri/gen/apple`, and defaults the output to `<project>/assets/bundles/builtin`. Pass
`--ios=<project>` to point at it explicitly. Add the `assets` folder to your Xcode target as a
**folder reference** (blue folder, not a group), regenerated from a **Run Script** build phase placed
**above** "Copy Bundle Resources":

```sh
# Run Script phase (above Copy Bundle Resources)
"$PROJECT_DIR/node_modules/.bin/wvb" builtin --ios="$SRCROOT"
```

For a **Tuist** project, declare the folder reference in `Project.swift`
(`resources: [.folderReference(path: "./assets")]`) and run `tuist generate`. At runtime, point the
source's `builtin_dir` at `Bundle.main.resourceURL` + `assets/bundles/builtin` (no extraction needed —
it's a real path).

> Add the generated staging directory to `.gitignore` (it's build output).

### Options

`wvb builtin --help` lists all flags. Notable ones: `--channel`, `--include`/`--exclude`,
`--config`, `--out` (override the preset default), and the mutually-exclusive presets `--tauri`,
`--android`, `--ios` — each auto-detects its project, or takes an explicit path (`--android=<module>`,
`--ios=<project>`, or `--tauri-dir <path>`).
