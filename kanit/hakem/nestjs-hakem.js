// NestJS hakemi: Bir NestJS uygulamasındaki bütün endpoint'leri, girdilerini, doğrulama
// kurallarını ve yetki bilgisini uygulamanın kendi TypeScript derleyicisiyle çıkarır.
// Sezgi'den bağımsızdır; kanıt testinde "doğru cevap" kaynağı olarak kullanılır.
//
// Kullanım: node nestjs-hakem.js <backend-klasörü> <çıktı.json>
'use strict';
const fs = require('fs');
const path = require('path');

const kok = path.resolve(process.argv[2]);
const cikti = process.argv[3];
// TypeScript derleyicisi: uygulamanın kendisi ya da TS_YOLU (bağımlılıkları kurulmamış üçüncü taraf depolar için)
const ts = require(process.env.TS_YOLU || path.join(kok, 'node_modules', 'typescript'));
const src = path.join(kok, 'src');

// Uygulama ayarları: genel önek ve genel guard'lar (main.ts, *.module.ts)
const mainTs = fs.readFileSync(path.join(src, 'main.ts'), 'utf8');
const onek = (mainTs.match(/setGlobalPrefix\(\s*['"`]([^'"`]+)['"`]/) || [])[1] || '';
const tumDosyalar = [];
(function gez(d) {
  for (const a of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, a.name);
    if (a.isDirectory()) gez(p);
    else if (a.name.endsWith('.ts') && !a.name.endsWith('.spec.ts')) tumDosyalar.push(p);
  }
})(src);
const genelGuardlar = [];
for (const f of tumDosyalar.filter(f => f.endsWith('.module.ts'))) {
  for (const m of fs.readFileSync(f, 'utf8').matchAll(/provide:\s*APP_GUARD\s*,\s*useClass:\s*(\w+)/g)) genelGuardlar.push(m[1]);
}

const kaynaklar = new Map();
const kaynak = f => {
  if (!kaynaklar.has(f)) kaynaklar.set(f, ts.createSourceFile(f, fs.readFileSync(f, 'utf8'), ts.ScriptTarget.Latest, true));
  return kaynaklar.get(f);
};
const satir = (sf, n) => sf.getLineAndCharacterOfPosition(n.getStart(sf)).line + 1;
const goreli = f => path.relative(kok, f).split(path.sep).join('/');

function dekoratorler(node) {
  const ds = ts.canHaveDecorators && ts.canHaveDecorators(node) ? ts.getDecorators(node) || [] : node.decorators || [];
  return ds.map(d => {
    const e = d.expression;
    if (ts.isCallExpression(e)) return { ad: e.expression.getText(), args: e.arguments.map(a => a.getText()), argNodes: e.arguments };
    return { ad: e.getText(), args: [], argNodes: [] };
  });
}
const metin = s => (s ? s.replace(/^['"`]|['"`]$/g, '') : '');

// İçe aktarmaları çöz: ad -> dosya
function iceAktarmalar(sf) {
  const m = new Map();
  for (const st of sf.statements) {
    if (!ts.isImportDeclaration(st) || !st.importClause) continue;
    const spec = st.moduleSpecifier.text;
    if (!spec.startsWith('.')) continue;
    let hedef = path.resolve(path.dirname(sf.fileName), spec);
    for (const aday of [hedef + '.ts', path.join(hedef, 'index.ts')]) if (fs.existsSync(aday)) { hedef = aday; break; }
    const nb = st.importClause.namedBindings;
    if (nb && ts.isNamedImports(nb)) for (const el of nb.elements) m.set(el.name.text, hedef);
  }
  return m;
}

// Sınıfı bul (önce içe aktarma, sonra aynı dosya, sonra bütün src)
function sinifBul(ad, sf) {
  const aday = [];
  const ia = iceAktarmalar(sf).get(ad);
  if (ia && fs.existsSync(ia)) aday.push(ia);
  aday.push(sf.fileName);
  for (const f of aday) {
    const s = kaynak(f);
    for (const st of s.statements) if (ts.isClassDeclaration(st) && st.name && st.name.text === ad) return { node: st, sf: s };
  }
  for (const f of tumDosyalar) {
    const s = kaynak(f);
    for (const st of s.statements) if (ts.isClassDeclaration(st) && st.name && st.name.text === ad) return { node: st, sf: s };
  }
  return null;
}

const KURAL_YOK = new Set(['ApiProperty', 'ApiPropertyOptional', 'ApiHideProperty', 'Type', 'Transform', 'Expose', 'Exclude']);
function dtoAlanlari(ad, sf, derinlik = 0) {
  if (!ad || derinlik > 4) return null;
  const b = sinifBul(ad, sf);
  if (!b) return { dto: ad, bulunamadi: true, alanlar: {} };
  const alanlar = {};
  let kalitim = null;
  for (const h of b.node.heritageClauses || []) {
    for (const t of h.types) {
      kalitim = t.getText(b.sf);
      // extends X  |  extends PartialType(X)  |  extends PickType(X, [...]) vb.
      const m = kalitim.match(/^(\w+)\((\w+)/);
      const ust = m ? m[2] : t.expression.getText(b.sf);
      const ustAlanlar = dtoAlanlari(ust, b.sf, derinlik + 1);
      if (ustAlanlar) for (const [k, v] of Object.entries(ustAlanlar.alanlar)) alanlar[k] = { ...v, kalitimdan: ust, ...(m && m[1] === 'PartialType' ? { zorunlu: false } : {}) };
    }
  }
  for (const uye of b.node.members) {
    if (!ts.isPropertyDeclaration(uye) || !uye.name) continue;
    const ds = dekoratorler(uye);
    const kurallar = ds.filter(d => !KURAL_YOK.has(d.ad)).map(d => (d.args.length ? `${d.ad}(${d.args.join(', ')})` : `${d.ad}()`));
    const opsiyonel = !!uye.questionToken || ds.some(d => d.ad === 'IsOptional' || d.ad === 'ValidateIf');
    alanlar[uye.name.getText(b.sf)] = {
      tip: uye.type ? uye.type.getText(b.sf) : null,
      zorunlu: !opsiyonel,
      kurallar,
      satir: satir(b.sf, uye),
    };
  }
  return { dto: ad, dosya: goreli(b.sf.fileName), satir: satir(b.sf, b.node), kalitim, alanlar };
}

const HTTP = new Set(['Get', 'Post', 'Put', 'Patch', 'Delete', 'All', 'Options', 'Head']);
const yolBirlestir = (...p) => '/' + p.map(x => x.replace(/^\/+|\/+$/g, '')).filter(Boolean).join('/');
const endpointler = [];

for (const f of tumDosyalar.filter(f => f.endsWith('.controller.ts'))) {
  const sf = kaynak(f);
  for (const st of sf.statements) {
    if (!ts.isClassDeclaration(st)) continue;
    const sd = dekoratorler(st);
    const ctrl = sd.find(d => d.ad === 'Controller');
    if (!ctrl) continue;
    let onekCtrl = metin(ctrl.args[0] || '');
    if (ctrl.argNodes[0] && ts.isObjectLiteralExpression(ctrl.argNodes[0])) {
      const p = ctrl.argNodes[0].properties.find(x => x.name && x.name.getText() === 'path');
      onekCtrl = p ? metin(p.initializer.getText()) : '';
    }
    const sinifGuard = sd.filter(d => d.ad === 'UseGuards').flatMap(d => d.args);
    const sinifPublic = sd.some(d => d.ad === 'Public');
    const rolDek = d => /Roles$/.test(d.ad); // Roles ve adı Roles ile biten özel rol işaretleri de
    const sinifRoller = sd.filter(rolDek).flatMap(d => d.args.map(metin));
    for (const uye of st.members) {
      if (!ts.isMethodDeclaration(uye)) continue;
      const md = dekoratorler(uye);
      const http = md.find(d => HTTP.has(d.ad));
      if (!http) continue;
      const guardlar = [...sinifGuard, ...md.filter(d => d.ad === 'UseGuards').flatMap(d => d.args)];
      const public_ = sinifPublic || md.some(d => d.ad === 'Public');
      const roller = [...sinifRoller, ...md.filter(rolDek).flatMap(d => d.args.map(metin))];
      let giris;
      if (public_) giris = guardlar.includes('OptionalJwtAuthGuard') ? 'istege_bagli' : 'herkese_acik';
      else if (genelGuardlar.includes('JwtAuthGuard') || guardlar.includes('JwtAuthGuard')) giris = guardlar.includes('OptionalJwtAuthGuard') ? 'istege_bagli' : 'gerekli';
      else giris = guardlar.includes('OptionalJwtAuthGuard') ? 'istege_bagli' : 'herkese_acik';
      const ozet = md.find(d => d.ad === 'ApiOperation');
      const govde = [], sorgu = [], parametre = [];
      for (const p of uye.parameters) {
        const pd = dekoratorler(p);
        const tip = p.type ? p.type.getText(sf) : null;
        for (const d of pd) {
          if (d.ad === 'Body') govde.push({ ad: metin(d.args[0]) || null, tip });
          else if (d.ad === 'Query') sorgu.push({ ad: metin(d.args[0]) || null, tip });
          else if (d.ad === 'Param') parametre.push({ ad: metin(d.args[0]) || null, tip });
        }
      }
      const dtoCoz = liste => liste.map(x => (!x.ad && x.tip && /^[A-Z]\w*$/.test(x.tip) ? { ...x, sema: dtoAlanlari(x.tip, sf) } : x));
      const yol = yolBirlestir(onek, onekCtrl, metin(http.args[0] || ''));
      endpointler.push({
        id: `${http.ad.toUpperCase()} ${yol}`,
        yontem: http.ad.toUpperCase(),
        yol,
        kontrolcu: st.name.text,
        fonksiyon: uye.name.getText(sf),
        dosya: goreli(f),
        satir: satir(sf, uye),
        giris,
        guardlar: [...new Set(guardlar)],
        roller: [...new Set(roller)],
        ozet: ozet ? (ozet.args[0].match(/summary:\s*['"`]([^'"`]+)/) || [])[1] || null : null,
        govde: dtoCoz(govde),
        sorgu: dtoCoz(sorgu),
        parametreler: parametre,
      });
    }
  }
}

endpointler.sort((a, b) => a.yol.localeCompare(b.yol) || a.yontem.localeCompare(b.yontem));
const ozet = {
  uretici: 'nestjs-hakem 0.1',
  uygulama: goreli(kok) || path.basename(kok),
  genel_onek: onek,
  genel_guardlar: genelGuardlar,
  endpoint_sayisi: endpointler.length,
  kontrolcu_sayisi: new Set(endpointler.map(e => e.kontrolcu)).size,
  giris: endpointler.reduce((m, e) => ((m[e.giris] = (m[e.giris] || 0) + 1), m), {}),
  govdeli: endpointler.filter(e => e.govde.length).length,
  dto_bulunamayan: endpointler.flatMap(e => [...e.govde, ...e.sorgu]).filter(x => x.sema && x.sema.bulunamadi).map(x => x.tip),
};
fs.writeFileSync(cikti, JSON.stringify({ ozet, endpointler }, null, 2));
console.log(JSON.stringify(ozet, null, 2));
