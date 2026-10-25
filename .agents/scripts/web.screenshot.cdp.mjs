// @agents-index Saves one PNG of the dashboard at a fixed size and color scheme, after the page has drawn its data.
//
// Usage: node web.screenshot.cdp.mjs CHROME URL PROFILE OUT --scheme dark|light --width W --height H
import { writeFile } from 'node:fs/promises';
import { setTimeout as sleep } from 'node:timers/promises';
import { openChrome } from './web.cdp.session.mjs';

const [chrome, url, profile, out, ...rest] = process.argv.slice(2);
const flag = (name, d) => { const i = rest.indexOf(name); return i >= 0 ? rest[i + 1] : d; };
const scheme = flag('--scheme', 'dark');
const width = Number(flag('--width', '1280'));
const height = Number(flag('--height', '1030'));

const s = await openChrome(chrome, profile);
try {
  await s.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 2, mobile: false });
  await s.send('Emulation.setScrollbarsHidden', { hidden: true });
  await s.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: scheme }] });
  await s.send('Page.enable');
  await s.send('Page.navigate', { url });
  // The page is ready when the daily log has rows, which needs the API data.
  const ready = await s.until("document.querySelectorAll('#log li, [data-section=log] li, .log li').length > 0 || document.readyState === 'complete' && performance.now() > 3000");
  if (!ready) throw new Error('the page did not draw its data');
  await sleep(500);
  const shot = await s.send('Page.captureScreenshot', { format: 'png', clip: { x: 0, y: 0, width, height, scale: 1 } });
  await writeFile(out, Buffer.from(shot.result.data, 'base64'));
  console.log(JSON.stringify({ out, width, height, scheme }));
} finally {
  s.stop();
}
