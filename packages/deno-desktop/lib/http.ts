import type { HttpResponse } from '@wvb/deno';

/** Statuses the `Response` constructor rejects a body for — e.g. a proxied `304 Not Modified`. */
const NULL_BODY_STATUS: ReadonlySet<number> = new Set([101, 103, 204, 205, 304]);

export function toResponse(resp: HttpResponse): Response {
  const headers = new Headers();
  for (const [name, value] of Object.entries(resp.headers)) {
    headers.set(name, value);
  }
  const body = NULL_BODY_STATUS.has(resp.status) ? null : resp.body;
  return new Response(body, { status: resp.status, headers });
}
