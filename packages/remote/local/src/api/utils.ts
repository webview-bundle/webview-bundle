export function normalizeBundleName(file: string): string {
  return file.replace(/([\\/\s])/g, '-').replace(/\.wvb$/, '');
}

export function isFileNotFoundError(e: unknown): boolean {
  return e instanceof Error && 'code' in e && e.code === 'ENOENT';
}
