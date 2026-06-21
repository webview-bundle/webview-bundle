export function unknownPlatform(): never {
  throw new Error('Unknown platform. Make sure native webview supports webview-bundle.');
}
