const { test, expect } = require('@playwright/test');
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const root = path.resolve(__dirname, '../..');
const fixtures = path.join(root, 'test-files');
const pdf = path.join(fixtures, 'PDF-PDF.pdf');
const generated = path.join(__dirname, '../.generated');
const twoPagePdf = path.join(generated, 'two-pages.pdf');

test.beforeAll(() => {
  fs.mkdirSync(generated, { recursive: true });
  if (!fs.existsSync(twoPagePdf)) {
    execFileSync('pdfunite', [pdf, pdf, twoPagePdf], { stdio: 'pipe' });
  }
});

function zipEntries(filePath) {
  return execFileSync('unzip', ['-Z1', filePath], { encoding: 'utf8' })
    .trim()
    .split(/\r?\n/)
    .filter(Boolean);
}

async function upload(request, filePath, target) {
  const response = await request.post('/api/upload', {
    multipart: {
      file: {
        name: path.basename(filePath),
        mimeType: 'application/pdf',
        buffer: fs.readFileSync(filePath)
      },
      to: target
    }
  });
  expect(response.ok()).toBeTruthy();
  return response.json();
}

async function waitForJob(request, job) {
  await expect.poll(async () => {
    const response = await request.get(`/api/jobs/${job.job_id}/status`, {
      headers: { 'X-Euoni-Job-Token': job.token }
    });
    expect(response.ok()).toBeTruthy();
    return (await response.json()).status;
  }, { timeout: 90_000 }).toBe('completed');
}

async function downloadJob(request, job) {
  const response = await request.get(`/api/jobs/${job.job_id}/download`, {
    headers: { 'X-Euoni-Job-Token': job.token }
  });
  expect(response.ok()).toBeTruthy();
  return {
    filename: response.headers()['content-disposition'],
    bytes: await response.body()
  };
}

test.describe('backend API integration', () => {
  test('reports health independently from the interface status text', async ({ request }) => {
    const response = await request.get('/api/health');
    expect(response.status()).toBe(200);
    await expect(response.json()).resolves.toEqual({ status: 'ok' });
  });

  test('converts every PDF ZIP target and authorizes downloads', async ({ request }) => {
    for (const target of ['png-zip', 'jpg-zip', 'html-zip']) {
      const input = target === 'html-zip' ? pdf : twoPagePdf;
      const job = await upload(request, input, target);

      const unauthorized = await request.get(`/api/jobs/${job.job_id}/status`);
      expect(unauthorized.status()).toBe(404);
      const unauthorizedDownload = await request.get(`/api/jobs/${job.job_id}/download`);
      expect(unauthorizedDownload.status()).toBe(404);

      await waitForJob(request, job);
      const result = await downloadJob(request, job);
      const archivePath = path.join(generated, `api-${target}.zip`);
      fs.writeFileSync(archivePath, result.bytes);
      expect(result.filename).toContain(`converted.${target.replace('-zip', '')}.zip`);

      const entries = zipEntries(archivePath);
      if (target === 'png-zip') {
        expect(entries).toEqual(['page-1.png', 'page-2.png']);
      } else if (target === 'jpg-zip') {
        expect(entries).toEqual(['page-1.jpg', 'page-2.jpg']);
      } else {
        expect(entries).toContain('index.html');
        const extracted = path.join(generated, 'api-html');
        fs.rmSync(extracted, { recursive: true, force: true });
        fs.mkdirSync(extracted, { recursive: true });
        execFileSync('unzip', ['-q', archivePath, '-d', extracted]);
        const html = fs.readFileSync(path.join(extracted, 'index.html'), 'utf8');
        for (const match of html.matchAll(/(?:src|href)=["']([^"']+)["']/g)) {
          const reference = match[1];
          if (/^(?:#|https?:|data:)/.test(reference)) continue;
          expect(fs.existsSync(path.resolve(extracted, reference))).toBeTruthy();
        }
      }
    }
  });
});
