const { test, expect } = require('@playwright/test');
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const root = path.resolve(__dirname, '../..');
const fixtures = path.join(root, 'test-files');
const pdf = path.join(fixtures, 'PDF-PDF.pdf');
const png = path.join(fixtures, 'IMAGEM1.png');
const generated = path.join(__dirname, '../.generated');
const twoPagePdf = path.join(generated, 'two-pages.pdf');

test.beforeAll(() => {
  fs.mkdirSync(generated, { recursive: true });
  if (!fs.existsSync(twoPagePdf)) {
    execFileSync('pdfunite', [pdf, pdf, twoPagePdf], { stdio: 'pipe' });
  }
});

test.beforeEach(async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('#server-status')).toHaveText('Conversor online');
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

test('loads the interface, reports health, and adapts to mobile', async ({ page }) => {
  await expect(page).toHaveTitle('Euoni / Conversor');
  await expect(page.locator('#page-title')).toHaveText('Arquivo-kun.');
  await expect(page.locator('#dropzone')).toBeVisible();
  await expect(page.locator('#format')).toBeDisabled();

  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator('#dropzone')).toBeVisible();
  const pageWidth = await page.evaluate(() => document.documentElement.scrollWidth);
  expect(pageWidth).toBeLessThanOrEqual(390);
});

test('shows source-specific format options, including PDF ZIP exports', async ({ page }) => {
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
    'PNG — todas as páginas (ZIP)',
    'JPG — todas as páginas (ZIP)',
    'DOCX',
    'TXT',
    'HTML',
    'HTML completo + recursos (ZIP)',
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

test('downloads all pages as a PNG ZIP from the browser', async ({ page }) => {
  await chooseFile(page, twoPagePdf);
  await convert(page, 'png-zip');
  const { download, bytes } = await waitForDownload(page);
  const archivePath = path.join(generated, 'downloaded-pages.zip');
  fs.writeFileSync(archivePath, bytes);

  expect(download.suggestedFilename()).toBe('converted.png.zip');
  expect(zipEntries(archivePath)).toEqual(['page-1.png', 'page-2.png']);
});

test('downloads all pages as a JPG ZIP from the browser', async ({ page }) => {
  await chooseFile(page, twoPagePdf);
  await convert(page, 'jpg-zip');
  const { download, bytes } = await waitForDownload(page);
  const archivePath = path.join(generated, 'downloaded-pages-jpg.zip');
  fs.writeFileSync(archivePath, bytes);

  expect(download.suggestedFilename()).toBe('converted.jpg.zip');
  expect(zipEntries(archivePath)).toEqual(['page-1.jpg', 'page-2.jpg']);
});

test('downloads HTML and its relative resources as a ZIP from the browser', async ({ page }) => {
  await chooseFile(page, pdf);
  await convert(page, 'html-zip');
  const { download, bytes } = await waitForDownload(page);
  const archivePath = path.join(generated, 'downloaded-html.zip');
  fs.writeFileSync(archivePath, bytes);

  expect(download.suggestedFilename()).toBe('converted.html.zip');
  const entries = zipEntries(archivePath);
  expect(entries).toContain('index.html');

  const extracted = path.join(generated, 'html');
  fs.rmSync(extracted, { recursive: true, force: true });
  fs.mkdirSync(extracted, { recursive: true });
  execFileSync('unzip', ['-q', archivePath, '-d', extracted]);
  const html = fs.readFileSync(path.join(extracted, 'index.html'), 'utf8');
  for (const match of html.matchAll(/(?:src|href)="([^"]+)"/g)) {
    const reference = match[1];
    if (/^(?:#|https?:|data:)/.test(reference)) continue;
    expect(fs.existsSync(path.resolve(extracted, reference))).toBeTruthy();
  }
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

test('shows unavailable state when the health endpoint becomes unreachable', async ({ page }) => {
  await page.route('**/api/health', route => route.abort());
  await page.reload();
  await expect(page.locator('#server-status')).toHaveText('Conversor indisponível');
  await page.unroute('**/api/health');
});
