import { test, expect } from '@playwright/test';
import { samples } from '../samples.mjs';

for (const [id, sample] of Object.entries(samples)) {
  test(`example ${id}: ${sample.label}`, async ({ page }) => {
    await page.goto('./');
    await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
    await page.getByLabel('Example', { exact: true }).selectOption(id);
    await expect(page.getByLabel('WLT source')).toHaveValue(sample.source);
    await expect(page.getByTestId('sample-description')).toHaveText(sample.description);
    await page.getByRole('button', { name: 'Compile', exact: true }).click();
    const status = page.getByTestId('compile-status');
    await expect(status).toHaveText(/^(Compiled successfully|Compilation error|Could not complete compilation)/);
    const expected = sample.expectation;
    if (expected.kind === 'diagnostic') {
      await expect(status).toHaveText('Compilation error');
      await expect(page.getByTestId('diagnostics')).toContainText(expected.code);
      if (expected.message) await expect(page.getByTestId('diagnostics')).toContainText(expected.message);
      await expect(page.getByTestId('sample-outcome')).toHaveText(`Expected diagnostic: ${expected.code}`);
      await expect(page.getByRole('button', { name: 'Run', exact: true })).toBeDisabled();
      return;
    }
    expect(await status.textContent(), await page.getByTestId('diagnostics').textContent()).toMatch(/^Compiled successfully/);
    await page.getByRole('button', { name: 'Run', exact: true }).click();
    if (expected.kind === 'run') {
      await expect(page.getByTestId('run-output')).toHaveText(expected.output);
      await expect(page.getByTestId('sample-outcome')).toHaveText(`Expected output: ${expected.output}`);
    } else {
      await expect(page.getByTestId('sample-outcome')).toContainText(`Compile only · ${expected.note}`);
      for (const name of expected.imports) await expect(page.getByTestId('run-output')).toContainText(name);
    }
  });
}

test('editing an example clears the advertised output until another example is selected', async ({ page }) => {
  await page.goto('./');
  await expect(page.getByTestId('compile-status')).toContainText('Compiled successfully');
  await page.getByLabel('WLT source').fill('(module)');
  await expect(page.getByTestId('sample-outcome')).toBeEmpty();
  await expect(page.getByText('Custom source', { exact: true })).toBeVisible();
  await page.getByLabel('Example', { exact: true }).selectOption('loop');
  await expect(page.getByTestId('sample-outcome')).toHaveText('Expected output: 42n');
});
