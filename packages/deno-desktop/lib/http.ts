import type { HttpResponse } from '@wvb/deno';

export function toResponse(resp: HttpResponse): Response {
  const headers = new Headers();
  for (const [name, value] of Object.entries(resp.headers)) {
    headers.set(name, value);
  }
  return new Response(resp.body, { status: resp.status, headers });
}
