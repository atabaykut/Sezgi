// Veri bağı hakemi: Her controller metodunun hangi Prisma modellerini okuduğunu ve yazdığını,
// uygulamanın kendi TypeScript derleyicisinin TİP DENETLEYİCİSİYLE çağrıları gerçek hedeflerine
// çözerek bulur. Sezgi'den bağımsızdır (Sezgi alan tipi + ad eşleştirmesi kullanır; hakem sembol çözümü).
// Düz fonksiyonlar, arayüzler, takma adlar dahil derleyicinin çözebildiği her çağrı izlenir.
//
// Kullanım: node veri-hakem.js <backend-klasörü> <çıktı.json> [derinlik=12]
'use strict';
const fs = require('fs');
const path = require('path');

const kok = path.resolve(process.argv[2]);
const cikti = process.argv[3];
const azami = Number(process.argv[4] || 12);
// TypeScript derleyicisi: uygulamanın kendisi ya da TS_YOLU (bağımlılıkları kurulmamış üçüncü taraf depolar için)
const ts = require(process.env.TS_YOLU || path.join(kok, 'node_modules', 'typescript'));

// Prisma modelleri: schema.prisma (ya da prisma/schema/*.prisma)
const semaMetni = [];
(function gez(d) {
  for (const a of fs.readdirSync(d, { withFileTypes: true })) {
    if (a.name === 'node_modules' || a.name.startsWith('.')) continue;
    const p = path.join(d, a.name);
    if (a.isDirectory()) gez(p);
    else if (a.name.endsWith('.prisma')) semaMetni.push(fs.readFileSync(p, 'utf8'));
  }
})(kok);
// Prisma'nın kendi ayarı: package.json -> "prisma": { "schema": "<dosya ya da klasör>" }
const pj = fs.existsSync(path.join(kok, 'package.json')) ? JSON.parse(fs.readFileSync(path.join(kok, 'package.json'), 'utf8')) : {};
// Şema uygulama klasörünün dışındaysa (ör. Nx monoreposunda kökteki prisma/): VERI_SEMA=<dosya>
if (process.env.VERI_SEMA) semaMetni.push(fs.readFileSync(process.env.VERI_SEMA, 'utf8'));
if (pj.prisma && pj.prisma.schema) {
  const h = path.resolve(kok, pj.prisma.schema);
  if (fs.existsSync(h)) {
    const dosyalar = fs.statSync(h).isDirectory() ? fs.readdirSync(h).filter(f => f.endsWith('.prisma')).map(f => path.join(h, f)) : [h];
    for (const f of dosyalar) semaMetni.push(fs.readFileSync(f, 'utf8'));
  }
}
const modeller = new Map();
for (const m of semaMetni.join('\n').matchAll(/^\s*model\s+(\w+)\s*\{/gm)) modeller.set(m[1][0].toLowerCase() + m[1].slice(1), m[1]);
const OKUMA = new Set(['findUnique', 'findUniqueOrThrow', 'findFirst', 'findFirstOrThrow', 'findMany', 'count', 'aggregate', 'groupBy']);
const YAZMA = new Set(['create', 'createMany', 'createManyAndReturn', 'update', 'updateMany', 'updateManyAndReturn', 'upsert', 'delete', 'deleteMany']);

// Nx gibi yapılarda tsconfig.json yalnızca referans içerir: VERI_TSCONFIG=<tsconfig.app.json>
const tsconfig = process.env.VERI_TSCONFIG ? path.resolve(process.env.VERI_TSCONFIG) : ts.findConfigFile(kok, ts.sys.fileExists, 'tsconfig.json');
const ayar = ts.parseJsonConfigFileContent(ts.readConfigFile(tsconfig, ts.sys.readFile).config, ts.sys, path.dirname(tsconfig));
const program = ts.createProgram(ayar.fileNames.filter(f => !f.endsWith('.spec.ts')), { ...ayar.options, noEmit: true });
const denetci = program.getTypeChecker();
const kokYol = kok.split(path.sep).join('/');
const projede = f => !f.includes('node_modules') && f.split(path.sep).join('/').startsWith(kokYol);

// Çağrının hedef bildirimlerini tip denetleyicisiyle bul
function hedefler(cagri) {
  let sembol = denetci.getSymbolAtLocation(ts.isPropertyAccessExpression(cagri.expression) ? cagri.expression.name : cagri.expression);
  if (!sembol) return [];
  if (sembol.flags & ts.SymbolFlags.Alias) sembol = denetci.getAliasedSymbol(sembol);
  return (sembol.declarations || []).filter(d => projede(d.getSourceFile().fileName) && d.body);
}

function izle(govde, bag, gorulen, derinlik) {
  (function gez(n) {
    if (ts.isCallExpression(n)) {
      const e = n.expression;
      // <alıcı>.<model>.<işlem>(...)
      if (ts.isPropertyAccessExpression(e) && ts.isPropertyAccessExpression(e.expression) && e.expression.expression.kind !== ts.SyntaxKind.ThisKeyword) {
        const model = modeller.get(e.expression.name.text);
        const islem = e.name.text;
        if (model && OKUMA.has(islem)) bag.okur.add(model);
        if (model && YAZMA.has(islem)) bag.yazar.add(model);
      }
      if (derinlik < azami) {
        for (const d of hedefler(n)) {
          if (gorulen.has(d)) continue;
          gorulen.add(d);
          izle(d.body, bag, gorulen, derinlik + 1);
        }
      }
    }
    ts.forEachChild(n, gez);
  })(govde);
}

const HTTP = new Set(['Get', 'Post', 'Put', 'Patch', 'Delete', 'All', 'Options', 'Head']);
const sonuc = {};
for (const sf of program.getSourceFiles()) {
  if (!sf.fileName.endsWith('.controller.ts') || sf.fileName.includes('node_modules')) continue;
  for (const st of sf.statements) {
    if (!ts.isClassDeclaration(st) || !st.name) continue;
    for (const uye of st.members) {
      if (!ts.isMethodDeclaration(uye) || !uye.body) continue;
      const ds = (ts.getDecorators ? ts.getDecorators(uye) : uye.decorators) || [];
      if (!ds.some(d => ts.isCallExpression(d.expression) && HTTP.has(d.expression.expression.getText(sf)))) continue;
      const bag = { okur: new Set(), yazar: new Set() };
      izle(uye.body, bag, new Set([uye]), 0);
      sonuc[`${st.name.text}.${uye.name.getText(sf)}`] = { okur: [...bag.okur].sort(), yazar: [...bag.yazar].sort() };
    }
  }
}
fs.writeFileSync(cikti, JSON.stringify({ uretici: 'veri-hakem 0.1', model_sayisi: modeller.size, derinlik: azami, metotlar: sonuc }, null, 2));
const n = Object.values(sonuc);
console.log(`model ${modeller.size} · metot ${n.length} · bağlı ${n.filter(x => x.okur.length || x.yazar.length).length}`);
