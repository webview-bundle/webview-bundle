import { invoke } from './invoke.js';

/** One bundle an update offers. */
export interface BundleUpdate {
  /** The name of the bundle. */
  name: string;
  /** The version of the bundle. */
  version: string;
  /**
   * Where the bundle is downloaded from. When absent the client falls back to the remote's
   * default url (`GET /bundles/:name/:version`).
   */
  downloadUrl?: string;
  /** Integrity value to verify the downloaded bundle. */
  integrity?: string;
  /** Arbitrary string-valued metadata for this bundle. */
  metadata?: Record<string, string>;
}

/** The update document the remote server serves. */
export interface Update {
  /** The unique id of this update. */
  id: string;
  /** When the update was created, formatted according to ISO 8601. */
  createdAt: string;
  /** The update model version this update is written against. */
  runtimeVersion: number;
  /** The bundles this update offers. */
  bundles: BundleUpdate[];
  /** Arbitrary string-valued metadata for this update. */
  metadata: Record<string, string>;
}

/** The signature the server sent for an update, parsed from the `wvb-signature` header. */
export interface UpdateSignature {
  /** The id of the key the update was signed with. */
  keyId: string;
  /** The signature value. */
  sig: string;
  /** The signature algorithm. */
  alg: string;
}

export interface RemoteUpdateResponse {
  /** Update information parsed from the response body. */
  update: Update;
  /** "ETag" header value, to pass back on the next request. */
  etag?: string;
  /** Signature information for this update. */
  signature?: UpdateSignature;
}

export interface RemoteGetUpdateOptions {
  /** The etag of the update previously received; sent as "if-none-match". */
  etag?: string;
  /** Release channel to fetch the update from. */
  channel?: string;
}

/**
 * Fetches the update document. Resolves `null` when the server answered `304 Not Modified`,
 * i.e. the update matching `etag` is still the current one.
 */
async function getUpdate(options?: RemoteGetUpdateOptions): Promise<RemoteUpdateResponse | null> {
  return invoke<RemoteUpdateResponse | null>('remoteGetUpdate', { options });
}

/** Downloads `url` into `filepath`. The file is not staged, so nothing is served from it yet. */
async function download(url: string, filepath: string): Promise<void> {
  return invoke<void>('remoteDownload', { url, filepath });
}

export interface RemoteApi {
  getUpdate: typeof getUpdate;
  download: typeof download;
}

export const remote: RemoteApi = {
  getUpdate,
  download,
};
