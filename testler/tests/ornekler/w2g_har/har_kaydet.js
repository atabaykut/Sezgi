// Playwright HAR: giriş → /items → Add Item → Save → satır menüsü → Delete → onay (Test 1 ile aynı akış)
const { chromium } = require('/opt/rea/node_modules/playwright-core');
(async () => {
  const exe = require('child_process').execSync('find /root/.cache/ms-playwright -name chrome-headless-shell -type f | head -1').toString().trim();
  const b = await chromium.launch({ executablePath: exe });
  const ctx = await b.newContext({ recordHar: { path: '/cikti/oturum.har', content: 'embed' } });
  const p = await ctx.newPage();
  const isaret = [];
  const im = ad => isaret.push({ ad, zaman: new Date().toISOString() });
  await p.goto('http://localhost:8000/login');
  await p.getByTestId('email-input').fill('admin@example.com');
  await p.getByTestId('password-input').fill(process.env.T1_PAROLA);
  im('giris'); await p.click('button[type=submit]'); await p.waitForTimeout(1500);
  await p.goto('http://localhost:8000/items'); await p.waitForTimeout(1000);
  im('add_item'); await p.getByRole('button', { name: 'Add Item' }).click();
  await p.fill('input[placeholder=Title]', 'HAR kaydi');
  im('save'); await p.getByRole('button', { name: 'Save' }).click(); await p.waitForTimeout(1200);
  const satir = p.locator('tr', { hasText: 'HAR kaydi' });
  im('menu'); await satir.locator('button').last().click(); await p.waitForTimeout(300);
  im('delete_menu'); await p.getByRole('menuitem', { name: /Delete/i }).click(); await p.waitForTimeout(300);
  im('delete'); await p.getByRole('button', { name: /^Delete$/ }).click(); await p.waitForTimeout(1200);
  require('fs').writeFileSync('/cikti/har_isaretleri.json', JSON.stringify(isaret, null, 1));
  await ctx.close(); await b.close();
})().catch(e => { console.error('HATA', e.message); process.exit(1); });
