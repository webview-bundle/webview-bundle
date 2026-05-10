export function normalizeBundleName(file: string): string {
  return file.replace(/([\\/\s])/g, '-').replace(/\.wvb$/, '');
}
