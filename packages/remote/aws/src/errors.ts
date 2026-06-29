export class BundleAlreadyUploadedError extends Error {
  override readonly name = 'BundleAlreadyUploadedError';

  constructor(
    readonly bundleName: string,
    readonly version: string
  ) {
    super(
      `"${bundleName}" Bundle already exists with version: ${version}. ` +
        'Use "force" (the "--force" CLI flag) to overwrite the existing version.'
    );
  }
}

export function isBundleAlreadyUploadedError(e: unknown): e is BundleAlreadyUploadedError {
  return (
    e instanceof BundleAlreadyUploadedError ||
    (e != null && typeof e === 'object' && (e as Error).name === 'BundleAlreadyUploadedError')
  );
}
