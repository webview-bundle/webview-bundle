import type { HTTPResponseError } from 'hono/types';

export type ExceptionType =
  | 'unsupported_update_protocol_version'
  | 'invalid_runtime_version'
  | 'invalid_expect_signature'
  | 'missing_expect_signature';

export type ExceptionData =
  | {
      type: 'unsupported_update_protocol_version';
      requestedProtocolVersion?: string;
    }
  | {
      type: 'invalid_expect_signature';
      requestedExpectSignature?: string;
      detail?: string;
    }
  | {
      type: 'invalid_runtime_version';
      requestedRuntimeVersion?: string;
    };

export type ExceptionDataOf<T extends ExceptionType> = {
  [K in ExceptionType]: Extract<ExceptionData, { type: K }> extends never
    ? null
    : Omit<Extract<ExceptionData, { type: K }>, 'type'>;
}[T];

export class Exception<T extends ExceptionType = ExceptionType>
  extends Error
  implements HTTPResponseError
{
  override readonly name = 'WebviewBundleRemoteException';
  readonly type: T;
  readonly data: ExceptionDataOf<T>;

  static getMessage<V extends ExceptionType = ExceptionType>(
    type: V,
    data: ExceptionDataOf<V>
  ): string {
    switch (type) {
      case 'unsupported_update_protocol_version':
        return `Unsupported update protocol version: ${(data as ExceptionDataOf<'unsupported_update_protocol_version'>).requestedProtocolVersion ?? '(none)'}`;
      case 'invalid_runtime_version':
        return `Invalid runtime version: ${(data as ExceptionDataOf<'invalid_runtime_version'>)?.requestedRuntimeVersion}`;
      case 'invalid_expect_signature':
        return `Invalid expect signature: ${(data as ExceptionDataOf<'invalid_expect_signature'>)?.requestedExpectSignature}`;
      case 'missing_expect_signature':
        return 'Missing expect signature';
    }
  }

  static getStatus(type: ExceptionType): number {
    switch (type) {
      case 'unsupported_update_protocol_version':
      case 'invalid_runtime_version':
      case 'invalid_expect_signature':
      case 'missing_expect_signature':
        return 400;
    }
  }

  constructor(type: T, data: ExceptionDataOf<T>, options?: ErrorOptions) {
    super(Exception.getMessage(type, data), options);
    this.type = type;
    this.data = data;
  }

  getResponse(): Response {
    return new Response(JSON.stringify({ message: this.message }), {
      status: Exception.getStatus(this.type),
      headers: {
        'content-type': 'application/json',
      },
    });
  }
}
