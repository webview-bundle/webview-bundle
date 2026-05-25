package dev.wvb.webview

import android.content.Context
import java.io.File

/**
 * Helpers for locating bundle directories on Android.
 *
 * [dev.wvb.BundleSource] reads bundles from real filesystem paths, but builtin
 * bundles ship inside the APK `assets`. [copyAssetDir] materializes an asset
 * directory into app storage so it can be used as a `builtinDir`.
 */
public object WebViewBundleAssets {
    /**
     * Recursively copies the asset directory [assetPath] into [destDir],
     * returning [destDir]. Existing files are overwritten unless [overwrite] is
     * `false`, in which case the copy is skipped when [destDir] already exists.
     */
    public fun copyAssetDir(
        context: Context,
        assetPath: String,
        destDir: File,
        overwrite: Boolean = true,
    ): File {
        if (!overwrite && destDir.exists()) {
            return destDir
        }
        val assets = context.assets
        val children = assets.list(assetPath).orEmpty()
        if (children.isEmpty()) {
            // Leaf node: copy the file.
            destDir.parentFile?.mkdirs()
            assets.open(assetPath).use { input ->
                destDir.outputStream().use { input.copyTo(it) }
            }
            return destDir
        }
        destDir.mkdirs()
        for (child in children) {
            copyAssetDir(context, "$assetPath/$child", File(destDir, child), overwrite = true)
        }
        return destDir
    }

    /** Default writable directory for remote (downloaded) bundles. */
    public fun defaultRemoteDir(context: Context): File =
        File(context.filesDir, "bundles").apply { mkdirs() }
}
