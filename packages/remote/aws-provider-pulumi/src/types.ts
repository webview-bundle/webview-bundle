export type LambdaRuntime = 'nodejs22.x' | 'nodejs24.x' | 'nodejs26.x';

export function getLambdaRuntimeTarget(runtime: LambdaRuntime): string {
  switch (runtime) {
    case 'nodejs22.x':
      return 'node22';
    case 'nodejs24.x':
      return 'node24';
    case 'nodejs26.x':
      return 'node26';
  }
}
