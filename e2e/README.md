# Euoni Converter E2E

The UI tests in `tests/euoni.spec.js` exercise the original browser interface
against the Docker Compose application at `http://localhost:8080`. They cover
Chromium and Firefox, format selection exposed by the original interface,
browser uploads, progress states, download bytes, error rendering, and a
mobile viewport.

The API integration tests in `tests/api.spec.js` use Playwright's
`APIRequestContext` so backend capabilities that are not exposed by the
restored interface remain covered. They verify `/api/health`, PDF upload,
`png-zip`, `jpg-zip`, and `html-zip` jobs, status polling, token authorization,
ZIP page entries, and HTML resource references.

From this directory:

```bash
npm install
npx playwright install chromium firefox
npm run test:docker
```

`test:docker` rebuilds and starts the Compose service before running both
browser projects. Reports, traces, screenshots, and videos are written under
`e2e/playwright-report` and `e2e/test-results`.
