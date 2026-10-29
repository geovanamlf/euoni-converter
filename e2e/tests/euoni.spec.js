const { test, expect } = require('@playwright/test');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '../..');
const fixtures = path.join(root, 'test-files');
const pdf = path.join(fixtures, 'PDF-PDF.pdf');
const png = path.join(fixtures, 'IMAGEM1.png');

test.beforeEach(async ({ page }) => {
  await page.goto('/');
});

async function chooseFile(page, filePath) {
  await page.locator('#file-input').setInputFiles(filePath);
  await expect(page.locator('#file-name')).toHaveText(path.basename(filePath));
  await expect(page.locator('#format')).toBeEnabled();
}

async function convert(page, target) {
  await page.locator('#format').selectOption(target);
  await page.locator('#convert-button').click();
  await expect(page.locator('.job-state').first()).toHaveText(/Na fila|Convertendo|Pronto/);
  await expect(page.locator('.job-progress').first()).toBeVisible();
  await expect(page.locator('.job-state').first()).toHaveText('Pronto', {
    timeout: 90_000
  });
}

async function waitForDownload(page) {
  const downloadPromise = page.waitForEvent('download');
  await page.locator('.download').first().click();
  const download = await downloadPromise;
  const target = await download.path();
  expect(target).toBeTruthy();
  return {
    download,
    bytes: fs.readFileSync(target)
  };
}

function zipEntries(filePath) {
  return execFileSync('unzip', ['-Z1', filePath], { encoding: 'utf8' })
    .trim()
    .split(/\r?\n/)
    .filter(Boolean);
}

test('loads the interface and adapts to mobile', async ({ page }) => {
  await expect(page).toHaveTitle('Euoni / Conversor');
  await expect(page.locator('#page-title')).toHaveText('Arquivo-kun.');
  await expect(page.locator('#dropzone')).toBeVisible();
  await expect(page.locator('#format')).toBeDisabled();

  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator('#dropzone')).toBeVisible();
  await expect(page.locator('.character-stage')).toBeVisible();
});

test('keeps the character stage present across wide viewports', async ({ page }) => {
  for (const viewport of [
    { width: 2560, height: 1080 },
    { width: 1920, height: 1080 },
    { width: 1366, height: 768 }
  ]) {
    await page.setViewportSize(viewport);
    await page.reload();
    const metrics = await page.evaluate(() => {
      const stage = document.querySelector('.character-stage').getBoundingClientRect();
      return {
        stageWidth: stage.width,
        stageHeight: stage.height
      };
    });

    expect(metrics.stageWidth).toBeGreaterThan(0);
    expect(metrics.stageHeight).toBeGreaterThan(0);
    await expect(page.locator('.character-stage')).toBeVisible();
  }
});

test('shows source-specific formats exposed by the original interface', async ({ page }) => {
  await chooseFile(page, png);
  await expect(page.locator('#format option')).toHaveText([
    'JPG',
    'WEBP',
    'AVIF',
    'HEIC',
    'PDF'
  ]);

  await chooseFile(page, pdf);
  await expect(page.locator('#format option')).toHaveText([
    'PNG',
    'JPG',
    'DOCX',
    'TXT',
    'HTML',
    'ODT',
    'RTF'
  ]);
});

test('uploads through the browser, exposes progress, and downloads a valid image', async ({
  page
}) => {
  await chooseFile(page, png);
  await convert(page, 'jpg');
  const { download, bytes } = await waitForDownload(page);

  expect(download.suggestedFilename()).toBe('converted.jpg');
  expect(bytes.subarray(0, 3).toString('hex')).toBe('ffd8ff');
});

test('shows upload validation errors in the browser', async ({ page }) => {
  await page.locator('#file-input').setInputFiles({
    name: 'broken.png',
    mimeType: 'image/png',
    buffer: Buffer.from('not an image')
  });
  await page.locator('#format').selectOption('jpg');
  await page.locator('#convert-button').click();
  await expect(page.locator('#convert-button')).toHaveText('file content does not match extension', {
    timeout: 2_000
  });
});
