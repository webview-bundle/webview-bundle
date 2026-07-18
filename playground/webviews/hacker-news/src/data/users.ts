import type { User } from './types';

export const users: Record<string, User> = {
  core_dev: {
    bio: 'Maintainer of @wvb/core. Rust, lz4, and offline-first evangelism.',
    karma: '12.4k',
    joined: '2y ago',
  },
  tauri_andy: {
    bio: 'Tauri contributor. Making app:// interception work everywhere.',
    karma: '8.1k',
    joined: '1y ago',
  },
  hashbrown: {
    bio: 'Checksums, hashing, and benchmarks. xxHash apologist.',
    karma: '3.7k',
    joined: '1y ago',
  },
  lz4_maxi: {
    bio: 'Compression nerd. Shipping smaller bundles every release.',
    karma: '5.2k',
    joined: '8mo ago',
  },
  byte_poet: {
    bio: 'I read hexdumps for fun. The magic number is art.',
    karma: '2.1k',
    joined: '1y ago',
  },
  offgrid: {
    bio: 'Field deployments where the network is a rumor.',
    karma: '1.9k',
    joined: '10mo ago',
  },
  determinist: {
    bio: "Reproducible builds or it didn't happen.",
    karma: '2.8k',
    joined: '1y ago',
  },
};
