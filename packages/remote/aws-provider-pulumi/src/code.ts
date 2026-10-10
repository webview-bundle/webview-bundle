import { rolldown } from 'rolldown';

export interface Code {
  fileName: string;
  content: string;
}

export interface GenerateCodeOptions {
  platform?: 'browser' | 'node' | 'neutral';
  external?: Array<string | RegExp>;
  format?: 'cjs' | 'esm';
  target?: string;
  define?: Record<string, string>;
  minify?: boolean;
  sourcemap?: boolean;
}

export async function generateCode(input: string, options?: GenerateCodeOptions): Promise<Code[]> {
  const bundle = await rolldown({
    input,
    platform: options?.platform,
    external: options?.external,
    transform: {
      define: options?.define,
      target: options?.target,
    },
  });
  const { output: outputs } = await bundle.generate({
    format: options?.format,
    sourcemap: options?.sourcemap,
    minify: options?.minify,
    entryFileNames: options?.format === 'esm' ? '[name].mjs' : '[name].js',
    chunkFileNames: options?.format === 'esm' ? '[name]-[hash].mjs' : '[name]-[hash].js',
    codeSplitting: false,
  });
  const codes: Code[] = [];
  for (const output of outputs) {
    if (output.type === 'chunk') {
      const code: Code = {
        fileName: output.fileName,
        content: output.code,
      };
      codes.push(code);
      if (output.map != null) {
        const code: Code = {
          fileName: output.sourcemapFileName!,
          content: output.map.toString(),
        };
        codes.push(code);
      }
    }
  }
  return codes;
}
