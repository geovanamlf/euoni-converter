# Euoni Converter E2E

These tests exercise the real browser interface against the Docker Compose
application at `http://localhost:8080`. They cover Chromium and Firefox,
format selection, browser uploads, progress states, download bytes, ZIP
entries, error rendering, health status, and a mobile viewport.

From this directory:

```bash
npm install
npx playwright install chromium firefox
npm run test:docker
```

`test:docker` rebuilds and starts the Compose service before running both
browser projects. Reports, traces, screenshots, and videos are written under
`e2e/playwright-report` and `e2e/test-results`.
