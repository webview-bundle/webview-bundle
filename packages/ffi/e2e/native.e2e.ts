import { test } from '@e2e-dev/mobile';
import { expect } from 'e2e';

test('FFI native suite passes', async ({ app, device, screen }) => {
  await device.installApp(undefined, { reinstall: true });
  await app.open();

  // SwiftUI can expose a toolbar item's ID on both its wrapper and its button.
  // Match the button role and the platform's label to select the actual control.
  await screen.getByRole('button', /^Run(?: tests)?$/i).tap();
  // Android resource IDs have a package prefix; iOS accessibility IDs do not.
  const summary = screen.getByTestId(/(^|:id\/)tv_summary$/);
  await expect(summary).toHaveText(/^\d+ passed, \d+ failed$/, { timeout: 180_000 });

  const text = (await summary.textContent())!;
  const [, passed, failed] = /^(\d+) passed, (\d+) failed$/.exec(text)!;
  expect(Number(passed) + Number(failed), 'native suite produced no results').toBeGreaterThan(0);

  // Read the native summary, not line-delimited output: accessibility text reads
  // normalize whitespace. Include the visible native failures in the report.
  if (Number(failed) > 0) {
    const output = (await screen.getByText(/\S/).allTextContents()).join('\n');
    expect(Number(failed), `${text}\n${output}`).toBe(0);
  }
});
