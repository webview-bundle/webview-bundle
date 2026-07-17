import type { CommentNode } from './types';

/** Threaded comments for the post-detail view (post #3, the streaming RFC). */
export const comments: CommentNode[] = [
  {
    id: 'c1',
    author: 'core_dev',
    base: 240,
    age: 2,
    body: `We prototyped this. The trick is that the Index is fully read first, so you can seek and inflate individual blocks lazily. First paint dropped from 180ms to 22ms on our largest bundle.`,
    children: [
      {
        id: 'c2',
        author: 'tauri_andy',
        op: true,
        base: 88,
        age: 2,
        body: `Right — since the lz4 block format is independently decodable per block, you don't need the whole stream. Did you hit any alignment issues on the last block?`,
        children: [
          {
            id: 'c3',
            author: 'core_dev',
            base: 54,
            age: 1,
            body: `Only on the last block. Padded it to 4 bytes and the reader stopped complaining.`,
            children: [
              {
                id: 'c4',
                author: 'byte_poet',
                base: 12,
                age: 1,
                body: `this is exactly the kind of detail that belongs in the spec README`,
              },
            ],
          },
        ],
      },
      {
        id: 'c5',
        author: 'hashbrown',
        base: 31,
        age: 1,
        body: `How does checksum verification interact with lazy inflate — do you verify per-block or whole-file?`,
        children: [
          {
            id: 'c6',
            author: 'core_dev',
            base: 29,
            age: 1,
            body: `Per-block xxHash-32 stored in the Index, with a whole-file fallback if the Index is legacy.`,
          },
        ],
      },
    ],
  },
  {
    id: 'c7',
    author: 'offgrid',
    base: 44,
    age: 3,
    body: `Counterpoint: for our offline kiosks the full inflate happens once at boot and never again. Streaming would add complexity we don't actually need.`,
    children: [
      {
        id: 'c8',
        author: 'tauri_andy',
        op: true,
        base: 20,
        age: 2,
        body: `Fair. This is mostly a win for large bundles on a cold start / very first launch.`,
      },
    ],
  },
  {
    id: 'c9',
    author: 'determinist',
    base: 9,
    age: 4,
    body: `Please keep this deterministic — streaming output must not reorder block boundaries or we lose byte-identical packs.`,
  },
];
