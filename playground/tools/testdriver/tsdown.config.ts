import { defineConfig } from 'tsdown';

export default defineConfig({
  entry: ['src/index.ts', 'src/playwright.ts', 'src/selenium.ts', 'src/appium.ts'],
  format: 'esm',
  dts: true,
  clean: true,
  treeshake: true,
  platform: 'node',
  target: 'node24',
});
