// @generated from `src/*.rs` by `cargo test` — do not edit.
/**
 * Options for `BundleBuilder.build`.
 */
export type BundleBuilderOptions = { header?: HeaderWriterOptions | null; index?: IndexWriterOptions | null; dataChecksum?: ChecksumWriteOptions | null }

/**
 * A bundle's header: format metadata read from the first bytes of a `.wvb` file.
 */
export type BundleHeader = { version: Version;
/**
 * Byte offset where the index section ends (the start of the data section).
 */
indexEndOffset: number;
/**
 * Size of the index section in bytes.
 */
indexSize: number }

/**
 * Bundle version with the source (builtin/remote) that provides it.
 */
export type BundleSourceVersion = { source: SourceKind; version: string }

/**
 * One bundle advertised by an update document.
 */
export type BundleUpdate = {
/**
 * Bundle name.
 */
name: string;
/**
 * Bundle version.
 */
version: string;
/**
 * Absolute download URL when the service overrides the default endpoint.
 */
downloadUrl?: string | null;
/**
 * Serialized integrity value for the downloaded bundle.
 */
integrity?: string | null;
/**
 * Provider-defined, string-valued bundle metadata.
 */
metadata?: Partial<{ [key in string]: string }> | null }

/**
 * How a bundle section's xxHash checksum is verified when that section is read. The same options
 * apply to the header, the index and each entry's data.
 *
 * This detects corruption, not tampering: the seed is not secret, so whatever can rewrite the
 * bytes can rewrite the checksum.
 */
export type ChecksumReadOptions = {
/**
 * Verify the section's checksum when it is read. Default: `true`.
 */
verify?: boolean | null;
/**
 * The seed the checksum was built with. Default: `0`.
 */
seed?: number | null }

/**
 * The seed an xxHash checksum is written with.
 */
export type ChecksumWriteOptions = { seed?: number | null }

/**
 * How each entry's data is read out of a bundle's data section.
 */
export type DataReadOptions = { checksum?: ChecksumReadOptions | null }

/**
 * How a bundle's header is read.
 */
export type HeaderReadOptions = { checksum?: ChecksumReadOptions | null }

/**
 * How a bundle's header section is written.
 */
export type HeaderWriterOptions = { checksum?: ChecksumWriteOptions | null }

/**
 * Which hostname segment names the bundle.
 */
export type HostnameSegment = "first" | "full" | "strip_suffix"

/**
 * HTTP method accepted by a protocol handler (case-insensitive on the wire).
 */
export type HttpMethod = "get" | "head" | "options" | "post" | "put" | "patch" | "delete" | "trace" | "connect"

/**
 * HTTP client options for a remote.
 */
export type HttpOptions = {
/**
 * Headers sent with every request.
 */
defaultHeaders?: Partial<{ [key in string]: string }> | null;
/**
 * User-Agent sent with every request.
 */
userAgent?: string | null;
/**
 * Total request timeout in milliseconds.
 */
timeout?: number | null;
/**
 * Response-body read timeout in milliseconds.
 */
readTimeout?: number | null;
/**
 * Connection-establishment timeout in milliseconds.
 */
connectTimeout?: number | null;
/**
 * Idle lifetime in milliseconds for pooled connections.
 */
poolIdleTimeout?: number | null;
/**
 * Maximum idle connections retained for one host.
 */
poolMaxIdlePerHost?: number | null;
/**
 * Whether redirect requests include a Referer header.
 */
referer?: boolean | null;
/**
 * Whether TCP sockets disable Nagle's algorithm.
 */
tcpNodelay?: boolean | null }

/**
 * Metadata for a single file in a bundle's index. Sizes are byte counts (`offset`/`len` over the
 * compressed data section; `contentLength` is the original, decompressed size).
 */
export type IndexEntry = { offset: number; len: number; isEmpty: boolean; contentType: string; contentLength: number; headers: Partial<{ [key in string]: string }> }

/**
 * How a bundle's index is read.
 */
export type IndexReadOptions = { checksum?: ChecksumReadOptions | null }

/**
 * How a bundle's index section is written.
 */
export type IndexWriterOptions = { checksum?: ChecksumWriteOptions | null }

export type IntegrityAlgorithm = "sha256" | "sha384" | "sha512"

/**
 * How a bundle's integrity metadata is treated when the integrity check runs.
 */
export type IntegrityPolicy = "strict" | "optional" | "off"

export type ManifestBundleItem = { name: string; version: string; status: ManifestBundleItemStatus; data: ManifestVersionData }

/**
 * Where a version stands in its bundle's lifecycle.
 */
export type ManifestBundleItemStatus = "current" | "previous" | "staged" | "orphan"

export type ManifestPruneResult = { name: string; prunedVersions: string[] }

export type ManifestRemoveData = { versions: string[]; force?: boolean | null }

export type ManifestRemoveResult = { name: string; version: string; kind: ManifestRemoveResultKind }

export type ManifestRemoveResultKind = "removed" | "not_exists" | "version_not_exists" | "in_use"

export type ManifestSetCurrentVersionResult = { name: string; version: string; kind: ManifestSetCurrentVersionResultKind }

export type ManifestSetCurrentVersionResultKind = "settled" | "not_exists" | "version_not_exists"

export type ManifestStageData = { version: string; data?: ManifestVersionData | null }

export type ManifestStageResult = { name: string; version: string; kind: ManifestStageResultKind }

export type ManifestStageResultKind = "staged" | "in_use"

/**
 * What the manifest records for one version of a bundle.
 */
export type ManifestVersionData = { integrity?: string | null; metadata?: Partial<{ [key in string]: string }> | null }

/**
 * `@wvb/node`'s `RemoteConfig` minus `onDownload`: Deno FFI cannot call back into JS from the
 * worker thread a `nonblocking` symbol runs on, so download progress is not reported here.
 */
export type RemoteConfig = {
/**
 * Base URL of the update service.
 */
baseUrl: string;
/**
 * Optional HTTP client settings.
 */
http?: HttpOptions | null }

/**
 * A successful update response returned by the remote service.
 */
export type RemoteUpdateResponse = {
/**
 * Parsed update document.
 */
update: Update;
/**
 * HTTP entity tag supplied by the remote service.
 */
etag?: string | null;
/**
 * Signature metadata supplied by the remote service.
 */
signature?: UpdateSignature | null }

/**
 * Digital signature algorithm for bundle verification. The wire strings match `@wvb/node`'s
 * `SignatureAlgorithm`.
 */
export type SignatureAlgorithm = "rsa-pkcs1-v1_5-sha256" | "rsa-pss-sha256" | "ecdsa-secp256r1" | "ecdsa-secp384r1" | "ed25519"

/**
 * Format of the public key used for signature verification.
 */
export type SignatureKeyFormat = "spki_der" | "spki_pem" | "pkcs1_der" | "pkcs1_pem" | "sec1" | "raw"

export type SourceConfig = { builtinDir: string; remoteDir: string; builtinManifestFilepath?: string | null; remoteManifestFilepath?: string | null; options?: SourceOptions | null }

/**
 * Which bundles are checked against the integrity recorded for them in the manifest when they are
 * loaded from disk.
 */
export type SourceIntegrityCheckMode =
/**
 * Verify both builtin and remote bundles.
 */
"all" |
/**
 * Check downloaded (remote) bundles only.
 */
"only_remote"

export type SourceIntegrityOptions = { policy?: IntegrityPolicy | null; checkMode?: SourceIntegrityCheckMode | null }

/**
 * The type of bundle source: builtin or remote.
 */
export type SourceKind = "builtin" | "remote"

export type SourceListItem = { source: SourceKind; item: ManifestBundleItem }

export type SourceOptions = { headerRead?: HeaderReadOptions | null; indexRead?: IndexReadOptions | null; dataRead?: DataReadOptions | null; integrity?: SourceIntegrityOptions | null; removeBundleChunkSize?: number | null }

/**
 * An atomically published set of bundle updates.
 */
export type Update = {
/**
 * Unique update identifier.
 */
id: string;
/**
 * ISO 8601 publication time.
 */
createdAt: string;
/**
 * Update-model version required to process this document.
 */
runtimeVersion: number;
/**
 * Bundles included in this update.
 */
bundles: BundleUpdate[];
/**
 * Provider-defined, string-valued update metadata.
 */
metadata: Partial<{ [key in string]: string }> }

/**
 * Signature metadata returned with an update document.
 */
export type UpdateSignature = {
/**
 * Identifier of the public key used to verify the signature.
 */
keyId: string;
/**
 * Base64-encoded signature of the raw update document.
 */
sig: string;
/**
 * Signature algorithm used for this signature.
 */
alg: string }

export type UpdaterDownloadOptions = { concurrency?: number | null; timeout?: number | null }

export type UpdaterGetUpdateOptions = {
/**
 * Require the update response to be signed by the key published under this id.
 */
expectSignatureKeyId?: string | null }

export type UpdaterInstallTarget = { name: string;
/**
 * The staged version to install. When omitted, the staged version recorded in the manifest is
 * used; when given, it has to match that staged version.
 */
version?: string | null }

export type UpdaterIntegrityOptions = { policy?: IntegrityPolicy | null; algorithm?: IntegrityAlgorithm | null }

export type UpdaterRollbackTarget = { name: string;
/**
 * The previous version to roll back to. When omitted, the previous version recorded in the
 * manifest is used; when given, it has to match that previous version.
 */
version?: string | null }

/**
 * How the file path in the bundle is resolved from the request uri.
 */
export type UriPathResolver = "exact" | "directory_index" | "html_extension"

/**
 * The `.wvb` bundle format version.
 */
export type Version = "v1"

