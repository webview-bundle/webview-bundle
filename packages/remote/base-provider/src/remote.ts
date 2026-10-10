import { type Env, Hono, type Schema } from 'hono';
import type { BlankEnv, BlankSchema } from 'hono/types';
import type { Config, ErrorContext } from './config.js';
import { Exception } from './exception.js';
import { parseRuntimeVersion } from './runtime.js';
import { parseExpectSignatureHeader, serializeSignature } from './signature.js';
import { isHTTPResponseError } from './utils.js';

const UPDATE_PROTOCOL_VERSION = '1';

export function remote<
  E extends Env = BlankEnv,
  S extends Schema = BlankSchema,
  BasePath extends string = '/',
>(config: Config<E, S, BasePath>): Hono<E, S, BasePath> {
  const {
    getUpdate,
    download,
    app = new Hono<E, S, BasePath>(),
    signaturePolicy = 'optional',
    disableIfNoneMatch = false,
    onException,
    onError,
  } = config;

  app.onError((error, c) => {
    const request = c.req.raw.clone();

    if (error instanceof Exception) {
      const customResponse = onException?.({
        request,
        exception: error,
      });
      return customResponse ?? error.getResponse();
    }

    const ctx: ErrorContext = {
      request,
      error,
    };
    const customResponse = onError?.(ctx);
    if (customResponse != null) {
      return customResponse;
    }

    if (isHTTPResponseError(error)) {
      return error.getResponse();
    }

    return c.json({ message: error.message }, 500);
  });

  app.get('/update', async c => {
    const protocolVersion = c.req.header('wvb-update-protocol-version');
    if (protocolVersion !== UPDATE_PROTOCOL_VERSION) {
      throw new Exception('unsupported_update_protocol_version', {
        requestedProtocolVersion: protocolVersion,
      });
    }

    const channel = c.req.header('wvb-update-channel');
    const update = await getUpdate({
      context: c,
      runtimeVersion: parseRuntimeVersion(c.req.header('wvb-runtime-version')),
      channel,
    });

    if (update == null) {
      return c.body(null, 204);
    }

    const expectSignature = parseExpectSignatureHeader(c.req.header('wvb-expect-signature'));
    const signature = update.signatures.find(
      x => x.id === expectSignature?.keyId && x.alg === expectSignature?.alg
    );
    const isMissing =
      signaturePolicy === 'strict'
        ? signature == null
        : signaturePolicy === 'optional'
          ? expectSignature != null && signature == null
          : false;
    if (isMissing) {
      const exception = new Exception('missing_expect_signature', null);
      const customResponse = onException?.({
        request: c.req.raw.clone(),
        exception,
      });
      return customResponse ?? exception.getResponse();
    }

    if (update.etag != null) {
      c.header('etag', update.etag);
    }

    if (!disableIfNoneMatch) {
      const ifNoneMatch = c.req.header('if-none-match');
      if (ifNoneMatch != null && ifNoneMatch === update.etag) {
        return c.body(null, 304);
      }
    }

    c.header('content-type', 'application/json; charset=UTF-8');
    if (signature != null) {
      c.header('wvb-signature', serializeSignature(signature));
    }

    return c.json(update.data, 200);
  });

  app.get('/bundles/:name/:version', async c => {
    const bundleName = c.req.param('name')!;
    const version = c.req.param('version')!;

    return await download({
      context: c,
      bundleName,
      version,
    });
  });

  return app;
}
