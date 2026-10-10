import type { Buffer } from 'node:buffer';

export interface UpdateVersionData {
  integrity?: string;
  metadata?: Record<string, string>;
}

export interface UploadParams {
  bundle: Buffer;
  name: string;
  version: string;
  versionData?: UpdateVersionData;
}

export interface UploadProgress {
  /** Number of bytes successfully transferred so far */
  loaded?: number;
  /** Total payload size in byres */
  total?: number;
  /** 1-based multipart part index currently being uploaded */
  part?: number;
}

export interface BaseUploader {
  _onUploadProgress?: (progress: UploadProgress) => void;
  upload(params: UploadParams): Promise<void>;
}
