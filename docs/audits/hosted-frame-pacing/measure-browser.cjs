// Usage: node measure-browser.cjs URL SAVE_JSON OUTPUT_DIR
// PLAYWRIGHT_MODULE may point to an existing Playwright installation.
const fs = require('node:fs/promises');
const path = require('node:path');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');

(async () => {
  const [url, savePath, output] = process.argv.slice(2);
  if (!url || !savePath || !output) throw new Error('Expected URL SAVE_JSON OUTPUT_DIR');
  await fs.mkdir(output, { recursive: true });
  const browser = await chromium.launch({
    headless: true,
    ...(process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {}),
    args: ['--enable-unsafe-swiftshader'],
  });
  try {
    const context = await browser.newContext({ viewport: { width: 800, height: 850 } });
    await context.addInitScript(save => {
      try { localStorage.setItem('pokered.save', save); } catch { /* about:blank */ }
      window.__tasks = 0;
      window.__draws = [];
      if (window.scheduler) {
        const post = scheduler.postTask.bind(scheduler);
        scheduler.postTask = (...args) => { window.__tasks++; return post(...args); };
      }
      for (const Type of [window.WebGLRenderingContext, window.WebGL2RenderingContext]) {
        if (!Type) continue;
        for (const key of ['drawArrays', 'drawElements', 'drawArraysInstanced', 'drawElementsInstanced']) {
          const original = Type.prototype[key];
          if (original) Type.prototype[key] = function (...args) {
            if (this.canvas?.id === 'game-canvas') window.__draws.push(performance.now());
            return original.apply(this, args);
          };
        }
      }
    }, await fs.readFile(savePath, 'utf8'));
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push({ url: page.url(), message: String(error) }));
    await page.goto(url, { waitUntil: 'domcontentloaded' });
    await page.locator('#fps-counter').filter({ hasText: 'FPS' }).waitFor();
    await page.locator('#game-canvas').click();
    await page.waitForTimeout(6500);
    for (let n = 0; n < 9; n++) {
      await page.keyboard.press('KeyZ', { delay: 80 });
      await page.waitForTimeout(650);
    }
    await page.waitForTimeout(1400);
    const cdp = await context.newCDPSession(page);
    await cdp.send('Performance.enable');
    await cdp.send('Profiler.enable');
    await cdp.send('Profiler.setSamplingInterval', { interval: 1000 });
    async function sample(name, seconds = 10) {
      await page.evaluate(() => { window.__tasks = 0; window.__draws = []; });
      const before = (await cdp.send('Performance.getMetrics')).metrics;
      await cdp.send('Profiler.start');
      await page.waitForTimeout(seconds * 1000);
      const profile = (await cdp.send('Profiler.stop')).profile;
      const after = (await cdp.send('Performance.getMetrics')).metrics;
      const metric = (values, key) => values.find(value => value.name === key).value;
      const result = await page.evaluate(() => ({
        tasks: window.__tasks, draws: window.__draws,
        fpsLabel: document.querySelector('#fps-counter').textContent,
        audioState: window.__pokered_audio_ctx?.state,
      }));
      result.elapsedSeconds = metric(after, 'Timestamp') - metric(before, 'Timestamp');
      result.mainThreadSeconds = metric(after, 'TaskDuration') - metric(before, 'TaskDuration');
      result.presentedFps = (result.draws.length - 1) * 1000 /
        (result.draws.at(-1) - result.draws[0]);
      await fs.writeFile(path.join(output, `${name}.json`), JSON.stringify(result, null, 2));
      await fs.writeFile(path.join(output, `${name}.cpuprofile`), JSON.stringify(profile));
      await page.screenshot({ path: path.join(output, `${name}.png`) });
      console.log(name, { ...result, draws: result.draws.length });
    }
    await sample('idle');
    await cdp.send('Emulation.setCPUThrottlingRate', { rate: 4 });
    await sample('idle-4x');
    await cdp.send('Emulation.setCPUThrottlingRate', { rate: 1 });
    await page.keyboard.down('ArrowDown');
    await sample('held-down', 4);
    await page.keyboard.up('ArrowDown');
    // Let the field finish its current movement/bump before testing a menu tap.
    await page.waitForTimeout(800);
    await page.keyboard.press('Space', { delay: 1 });
    await page.waitForTimeout(1600);
    await page.screenshot({ path: path.join(output, 'menu.png') });
    await page.keyboard.press('KeyX', { delay: 1 });
    await page.waitForTimeout(500);
    await page.screenshot({ path: path.join(output, 'menu-return.png') });
    await fs.writeFile(path.join(output, 'errors.json'), JSON.stringify(errors, null, 2));
    if (errors.length) throw new Error(JSON.stringify(errors));
  } finally {
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
