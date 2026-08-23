/** biome-ignore-all lint/correctness/useImportExtensions: allow .cjs */
export * from '../binding.cjs';
export {
  getWebviewBundleError,
  isWebviewBundleError,
  type WebviewBundleError,
} from './error.js';
