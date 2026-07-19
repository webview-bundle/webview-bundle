// @generated from `src/*.rs` by `cargo test` — do not edit.
/**
 * The `.wvb` bundle format version.
 */
export type BundleFormatVersion = "v1"

/**
 * A bundle's header: format metadata read from the first bytes of a `.wvb` file.
 */
export type BundleHeader = { version: BundleFormatVersion; 
/**
 * Byte offset where the index section ends (the start of the data section).
 */
indexEndOffset: number; 
/**
 * Size of the index section in bytes.
 */
indexSize: number }

/**
 * Metadata for a bundle version in the manifest.
 */
export type BundleManifestMetadata = { etag?: string | null; integrity?: string | null; signature?: string | null; lastModified?: string | null }

/**
 * The type of bundle source: builtin or remote. (`BundleSourceKind` in core; the deno binding has
 * always spelled it `BundleSourceType` on the wire.)
 */
export type BundleSourceType = "builtin" | "remote"

/**
 * Which bundles a load-time verification applies to.
 */
export type BundleSourceVerifyMode = "all" | "onlyRemote"

/**
 * Bundle version with the source (builtin/remote) that provides it.
 */
export type BundleSourceVersion = { type: BundleSourceType; version: string }

/**
 * The result of an updater `getUpdate`: the available remote version plus the local one.
 */
export type BundleUpdateInfo = { name: string; version: string; localVersion?: string | null; isAvailable: boolean; etag?: string | null; integrity?: string | null; signature?: string | null; lastModified?: string | null }

/**
 * Which hostname segment is used as the bundle name.
 */
export type HostnameSegment = "first" | "full" | "stripSuffix"

/**
 * HTTP method accepted by a protocol handler (case-insensitive on the wire).
 */
export type HttpMethod = "get" | "head" | "options" | "post" | "put" | "patch" | "delete"

/**
 * Metadata for a single file in a bundle's index. Sizes are byte counts (`offset`/`len` over the
 * compressed data section; `contentLength` is the original, decompressed size).
 */
export type IndexEntry = { offset: number; len: number; isEmpty: boolean; contentType: string; contentLength: number; headers: Partial<{ [key in string]: string }> }

export type IntegrityAlgorithm = "sha256" | "sha384" | "sha512"

/**
 * How a bundle's integrity metadata is treated when the integrity check runs.
 */
export type IntegrityPolicy = "strict" | "optional" | "off"

/**
 * A bundle entry from a source `listBundles`. Flat by design: core nests the manifest fields under
 * an `item`, but every binding's wire (and `@wvb/node`) flattens them onto the parent.
 */
export type ListBundleItem = { type: BundleSourceType; name: string; version: string; current: boolean; metadata: BundleManifestMetadata }

/**
 * Bundle list info from the remote server.
 */
export type ListRemoteBundleInfo = { name: string; version: string }

/**
 * How the file path in the bundle is resolved from the request uri.
 */
export type PathResolver = "exact" | "directoryIndex" | "htmlExtension"

/**
 * Bundle info from the remote server.
 */
export type RemoteBundleInfo = { name: string; version: string; etag?: string | null; integrity?: string | null; signature?: string | null; lastModified?: string | null }

/**
 * Digital signature algorithm for bundle verification. The wire strings match `@wvb/node`'s
 * napi-generated `SignatureAlgorithm` (note the capital `R` in `ecdsaSecp256R1`).
 */
export type SignatureAlgorithm = "ecdsaSecp256R1" | "ecdsaSecp384R1" | "ed25519" | "rsaPkcs1V15" | "rsaPss"

/**
 * Format of the public key used for signature verification.
 */
export type VerifyingKeyFormat = "spkiDer" | "spkiPem" | "pkcs1Der" | "pkcs1Pem" | "sec1" | "raw"

