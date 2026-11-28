// Render the boot screen images from the logo SVGs, with Playwright's
// Chromium: bgrt-fallback.png (the mark, 192 px square) and watermark.png
// ("Celenn OS" alone, 70 px high, as in the wallpaper's corner: the mark is
// a "C", so the mark then "Celenn" read as a double C, and the middle of
// the screen already shows the mark when the PC has no logo of its own).
// Usage: node render.js (from this folder)
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const logo = (name) => fs.readFileSync(path.join(__dirname, '../logo', name), 'utf8');
const theme = path.join(__dirname, '../../system_files/usr/share/plymouth/themes/celenn');
const HEIGHT = 70; // of the watermark

(async () => {
  const browser = await chromium.launch();
  const render = async (svg, width, height, out) => {
    const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
    await page.setContent('<html><body style="margin:0;overflow:hidden;background:transparent">' + svg + '</body></html>');
    await page.evaluate(() => { document.querySelector('svg').style.cssText = 'width:100vw;height:100vh;display:block'; });
    await page.screenshot({ path: path.join(theme, out), omitBackground: true });
    await page.close();
  };
  await render(logo('celenn-logo.svg'), 192, 192, 'bgrt-fallback.png');
  // The name only: the logo's text runs from x 118 to its width less 6 (logo
  // units, see branding/logo/build.py), kept with 6 units on each side
  const logoSvg = logo('celenn-os-logo-horizontal-dark.svg');
  const [, , w, h] = logoSvg.match(/viewBox="([^"]+)"/)[1].split(' ').map(Number);
  const left = 118 - 6;
  const name = logoSvg.replace(/viewBox="[^"]+"/, `viewBox="${left} 0 ${w - left} ${h}"`);
  await render(name, Math.round((w - left) * HEIGHT / h), HEIGHT, 'watermark.png');
  await browser.close();
})();
