export interface Variables {
  baseDir: string;
  proxy?: {
    endpoint: string;
    cachePrefix?: string;
  };
}
