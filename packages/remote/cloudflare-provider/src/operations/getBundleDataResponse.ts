import type { Context } from '../context.js';

export async function getBundleDataResponse(
  context: Context,
  bundleName: string,
  version: string
): Promise<Response | null> {
  const key = `bundles/${bundleName}/${bundleName}_${version}.wvb`;
  const obj = await context.r2.get(key);
  if (obj == null) {
    return null;
  }
  return new Response(obj.body, {
    status: 200,
    headers: makeHeaders(obj, bundleName, version),
  });
}

function makeHeaders(obj: R2ObjectBody, bundleName: string, version: string): Headers {
  const headers = new Headers();
  for (const [name, value] of Object.entries(obj.customMetadata ?? {})) {
    if (name.startsWith('webview-bundle-')) {
      headers.set(name, value);
    }
  }
  const contentType = obj.httpMetadata?.contentType ?? 'application/webview-bundle';
  headers.set('content-type', contentType);
  if (obj.httpMetadata?.contentDisposition != null) {
    headers.set('content-disposition', obj.httpMetadata.contentDisposition);
  }
  if (obj.httpMetadata?.cacheControl != null) {
    headers.set('cache-control', obj.httpMetadata.cacheControl);
  }
  headers.set('webview-bundle-name', bundleName);
  headers.set('webview-bundle-version', version);
  return headers;
}
