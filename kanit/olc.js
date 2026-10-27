// Kanıt testi ölçücüsü. Görevleri bir yapay zekâ ajanına çözdürür, token / süre / maliyet ölçer
// ve cevapları hakemin beklenen cevaplarıyla puanlar.
//
// Kullanım: node olc.js <duzen> [--model claude-sonnet-5-5] [--paralel 4] [--sadece G01,G02]
//   duzen A : ajan uygulamanın kod klasöründe, yalnız okuma araçlarıyla (Read, Grep, Glob)
//   duzen D : A + codebase-memory-mcp MCP sunucusu (kod haritası aracı)
//   duzen C : Sezgi (ileride)
'use strict';
const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');

const argv = process.argv.slice(2);
const duzen = argv[0];
const opt = (ad, vars) => { const i = argv.indexOf('--' + ad); return i >= 0 ? argv[i + 1] : vars; };
const MODEL = opt('model', 'claude-sonnet-5-5');
const PARALEL = +opt('paralel', 4);
const SADECE = opt('sadece', null);
const YENIDEN = argv.includes('--yeniden-puanla');
// Ölçülecek uygulama (özel uygulamanın yolu kodda tutulmaz: --uygulama ya da SEZGI_OLC_UYGULAMA):
//   --uygulama <kod klasörü> --gorevler <gorevler.json> --harita <sezgi çıktısı> --openapi <B için> --etiket <ad>
const UYGULAMA = opt('uygulama', process.env.SEZGI_OLC_UYGULAMA);
if (!UYGULAMA) { console.error('--uygulama <kod klasörü> gerekli'); process.exit(2); }
// D düzeni: codebase-memory-mcp'nin bu uygulama için kullandığı proje adı
const D_PROJE = opt('proje', process.env.SEZGI_OLC_D_PROJE || '');
const DIZIN = __dirname;
const GOREV_DOSYASI = opt('gorevler', process.env.SEZGI_OLC_GOREVLER);
if (!GOREV_DOSYASI) { console.error('--gorevler <gorevler.json> gerekli'); process.exit(2); }
// Ajan uygulama klasöründe çalışır; MCP sunucularına verilen yollar mutlak olmalı
const HARITA = path.resolve(opt('harita', process.env.SEZGI_OLC_HARITA || '.sezgi'));
const OPENAPI = path.resolve(opt('openapi', process.env.SEZGI_OLC_OPENAPI || 'openapi.json'));
const ETIKET = opt('etiket', '');
const gorevSeti = JSON.parse(fs.readFileSync(GOREV_DOSYASI, 'utf8'));
const { gorevler } = gorevSeti;
const secili = SADECE ? gorevler.filter(g => SADECE.split(',').includes(g.id)) : gorevler;
const CIKTI = path.join(DIZIN, 'sonuclar', `${ETIKET ? ETIKET + '-' : ''}${duzen}-${MODEL}`);
fs.mkdirSync(CIKTI, { recursive: true });

const ORTAK_C = `Bu uygulamanın Sezgi haritası sana MCP aracı olarak bağlı (sezgi: kimsin, ara, tani). Bir operasyon çalışanı senden bu uygulamayla ilgili bir iş istiyor.
Görevin: isteği yerine getirmek için uygulamanın HTTP API'sinde hangi çağrının yapılması gerektiğini bulmak. Çağrıyı yapma, sadece bul.
Kurallar: Cevabını yalnızca tek bir JSON nesnesi olarak ver, başka metin yazma.`;
const ORTAK = `Bu klasörde bir uygulamanın backend kodu var. Bir operasyon çalışanı senden bu uygulamayla ilgili bir iş istiyor.
Görevin: isteği yerine getirmek için uygulamanın HTTP API'sinde hangi çağrının yapılması gerektiğini bulmak. Çağrıyı yapma, sadece bul.
Kurallar: Hiçbir dosyayı değiştirme. Cevabını yalnızca tek bir JSON nesnesi olarak ver, başka metin yazma.`;

const BICIM = {
  cagri: `JSON biçimi:
{"yontem": "GET|POST|PUT|PATCH|DELETE", "yol": "/tam/adres (genel önek dahil, yol parametrelerini {} ile yaz)", "giris": "gerekli|herkese_acik|istege_bagli", "roller": ["${gorevSeti.roller_aciklama || 'gerekli roller, yoksa boş'}"], "zorunlu_alanlar": ["istek gövdesindeki zorunlu alan adları, yoksa boş"]}`,
  kural: `JSON biçimi:
{"cevap": "kısa ve kesin cevap"}`,
};

function arac(duzen) {
  if (duzen === 'B') {
    // B: kod okuma aracı YOK; yapay zekânın elinde uygulamanın kendi OpenAPI dokümanı (Sezgi'yle aynı araç arayüzü).
    const cfg = path.join(CIKTI, 'mcp.json');
    const ileri = p => p.split(path.sep).join('/');
    fs.writeFileSync(cfg, JSON.stringify({ mcpServers: { openapi: { command: 'node', args: [ileri(path.join(DIZIN, 'b-mcp.js')), ileri(OPENAPI)] } } }));
    return ['-p', '--model', MODEL, '--output-format', 'json', '--tools', '""', '--strict-mcp-config', '--mcp-config', cfg, '--allowedTools', 'mcp__openapi'];
  }
  if (duzen === 'C') {
    // C: kod okuma aracı YOK; yapay zekânın elinde yalnızca Sezgi var.
    const cfg = path.join(CIKTI, 'mcp.json');
    const ileri = p => p.split(path.sep).join('/');
    const exe = ileri(process.env.SEZGI_IKILI || path.join(DIZIN, '..', ...(fs.existsSync(path.join(DIZIN, '..', 'Cargo.toml')) ? [] : ['sezgi']), 'target', 'release', process.platform === 'win32' ? 'sezgi.exe' : 'sezgi'));
    fs.writeFileSync(cfg, JSON.stringify({ mcpServers: { sezgi: { command: exe, args: ['mcp', ileri(HARITA)] } } }));
    return ['-p', '--model', MODEL, '--output-format', 'json', '--tools', '""', '--strict-mcp-config', '--mcp-config', cfg, '--allowedTools', 'mcp__sezgi'];
  }
  const args = ['-p', '--model', MODEL, '--output-format', 'json', '--tools', 'Read', 'Grep', 'Glob', '--strict-mcp-config'];
  if (duzen === 'D') {
    // codebase-memory-mcp ikilisinin yolu ortamdan: SEZGI_OLC_CBM
    const exe = process.env.SEZGI_OLC_CBM || 'codebase-memory-mcp';
    const cfg = path.join(CIKTI, 'mcp.json');
    fs.writeFileSync(cfg, JSON.stringify({ mcpServers: { 'codebase-memory-mcp': { command: exe, args: [] } } }));
    args.push('--mcp-config', cfg, '--allowedTools', 'mcp__codebase-memory-mcp');
  }
  return args;
}

function calistir(g) {
  return new Promise(ok => {
    const istem = `${duzen === 'C' ? ORTAK_C : duzen === 'B' ? ORTAK_C.replace('Sezgi haritası sana MCP aracı olarak bağlı (sezgi: kimsin, ara, tani)', 'OpenAPI dokümanı sana MCP aracı olarak bağlı (openapi: kimsin, ara, tani)') : ORTAK}\n\n${duzen === 'D' ? `Kod haritası için codebase-memory-mcp araçlarını kullanabilirsin; proje adı: ${D_PROJE}.\n\n` : ''}${gorevSeti.dogrulama_talimati ? gorevSeti.dogrulama_talimati + '\n\n' : ''}İstek: ${g.soru}\n\n${BICIM[g.tur]}`;
    const t0 = Date.now();
    const p = spawn('claude', arac(duzen), { cwd: UYGULAMA, shell: true });
    p.stdin.end(istem);
    let out = '', err = '';
    p.stdout.on('data', d => (out += d));
    p.stderr.on('data', d => (err += d));
    p.on('close', () => {
      let j = null;
      try { j = JSON.parse(out); } catch {}
      const u = (j && j.usage) || {};
      const sonuc = {
        id: g.id,
        sure_ms: Date.now() - t0,
        tur_sayisi: j && j.num_turns,
        token_girdi: (u.input_tokens || 0) + (u.cache_creation_input_tokens || 0) + (u.cache_read_input_tokens || 0),
        token_cikti: u.output_tokens || 0,
        maliyet_usd: j && j.total_cost_usd,
        ham: j ? j.result : out.slice(0, 2000),
        hata: j ? null : err.slice(0, 1000),
      };
      fs.writeFileSync(path.join(CIKTI, `${g.id}.json`), JSON.stringify(sonuc, null, 2));
      ok(sonuc);
    });
  });
}

// ---------- Puanlama ----------
const yolNorm = y => (y || '').trim().replace(/[{:][^/}]*}?/g, '{}').replace(/\/+$/, '').toLowerCase();
const rolNorm = r => (r || []).map(x => String(x).replace(/^UserRole\./i, '').toUpperCase()).sort();
function jsonCikar(metin) {
  if (!metin) return null;
  const m = metin.match(/\{[\s\S]*\}/);
  try { return m ? JSON.parse(m[0]) : null; } catch { return null; }
}
function puanla(g, s) {
  const c = jsonCikar(s.ham);
  if (!c) return { puan: 0, ayrinti: 'JSON yok' };
  if (g.tur === 'kural') {
    const cevap = String(c.cevap || '').toLowerCase();
    const eksik = g.beklenen.anahtarlar.filter(a => !new RegExp(`(^|[^0-9a-z])${a.toLowerCase()}([^0-9a-z]|$)`).test(cevap));
    return { puan: eksik.length ? 0 : 1, ayrinti: eksik.length ? `eksik: ${eksik}` : 'tam' };
  }
  const b = g.beklenen;
  const kapi = String(c.yontem || '').toUpperCase() === b.yontem && yolNorm(c.yol) === yolNorm(b.yol);
  const giris = c.giris === b.giris;
  const roller = JSON.stringify(rolNorm(c.roller)) === JSON.stringify(b.roller);
  const verilen = new Set((c.zorunlu_alanlar || []).map(String));
  const beklenen = new Set(b.zorunlu_alanlar);
  const alanlar = verilen.size === beklenen.size && [...beklenen].every(a => verilen.has(a));
  // Kapı yanlışsa geri kalanın anlamı yoktur.
  const puan = kapi ? (giris ? 0.2 : 0) + (roller ? 0.2 : 0) + (alanlar ? 0.2 : 0) + 0.4 : 0;
  return { puan: +puan.toFixed(2), kapi, giris, roller, alanlar, verilen: c };
}

(async () => {
  console.log(`Düzen ${duzen} · model ${MODEL} · ${secili.length} görev · paralel ${PARALEL}`);
  const kuyruk = [...secili];
  const sonuclar = [];
  await Promise.all(Array.from({ length: PARALEL }, async () => {
    while (kuyruk.length) {
      const g = kuyruk.shift();
      const s = YENIDEN ? JSON.parse(fs.readFileSync(path.join(CIKTI, `${g.id}.json`), 'utf8')) : await calistir(g);
      const p = puanla(g, s);
      sonuclar.push({ ...s, ...p });
      console.log(`${g.id} puan=${p.puan} token=${s.token_girdi}+${s.token_cikti} süre=${(s.sure_ms / 1000).toFixed(0)}sn tur=${s.tur_sayisi} ${s.hata ? 'HATA' : ''}`);
    }
  }));
  sonuclar.sort((a, b) => a.id.localeCompare(b.id));
  const top = (k) => sonuclar.reduce((t, s) => t + (s[k] || 0), 0);
  const ozet = {
    duzen, model: MODEL, gorev: sonuclar.length,
    ortalama_puan: +(top('puan') / sonuclar.length).toFixed(3),
    kapi_dogru: sonuclar.filter(s => s.kapi).length,
    tam_dogru: sonuclar.filter(s => s.puan === 1).length,
    toplam_token_girdi: top('token_girdi'),
    toplam_token_cikti: top('token_cikti'),
    ortalama_token: Math.round((top('token_girdi') + top('token_cikti')) / sonuclar.length),
    ortalama_sure_sn: +(top('sure_ms') / sonuclar.length / 1000).toFixed(1),
    toplam_maliyet_usd: +top('maliyet_usd').toFixed(2),
    ortalama_tur: +(top('tur_sayisi') / sonuclar.length).toFixed(1),
  };
  fs.writeFileSync(path.join(CIKTI, '_ozet.json'), JSON.stringify({ ozet, sonuclar }, null, 2));
  console.log(JSON.stringify(ozet, null, 2));
})();
