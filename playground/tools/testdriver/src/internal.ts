/** Join a base URL with an in-app path, tolerant of custom schemes (`app://…`). */
export function joinUrl(baseURL: string, path: string): string {
  return baseURL.replace(/\/+$/, '') + (path.startsWith('/') ? path : `/${path}`);
}
