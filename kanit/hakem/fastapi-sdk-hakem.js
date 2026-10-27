// FastAPI hakemi: uygulamanın kendi OpenAPI belgesinden üretilmiş istemci (hey-api: sdk.gen.ts + types.gen.ts) okunur.
// FastAPI OpenAPI'yi kodundan kendisi üretir; üretilmiş istemci o belgenin depoya işlenmiş hâlidir. Bu yüzden hakem
// uygulamanın kendi beyanıdır ve Sezgi'nin kodundan bağımsızdır. Uygulama kodu çalıştırılmaz.
// Çıktı `karsilastir.js`'in hakem biçimindedir: endpoint, yöntem, yol, giriş (security), gövde ve sorgu alanları,
// zorunluluk. (Doğrulama kuralları türlerde olmadığı için karşılaştırılmaz.)
// Kullanım: TS_YOLU=<typescript> node fastapi-sdk-hakem.js <client-klasörü> <hakem.json>
'use strict';
const fs = require('fs');
const path = require('path');
const ts = require(process.env.TS_YOLU || 'typescript');

const [klasor, cikti] = process.argv.slice(2);
const oku = (ad) => ts.createSourceFile(ad, fs.readFileSync(path.join(klasor, ad), 'utf8'), ts.ScriptTarget.Latest, true);
const sdk = oku('sdk.gen.ts');
const tipler = oku('types.gen.ts');

// types.gen.ts: tip adı -> tip düğümü
const tipDizini = new Map();
tipler.forEachChild((n) => {
  if (ts.isTypeAliasDeclaration(n)) tipDizini.set(n.name.text, n.type);
});
const uyeler = (tip) => {
  // Tip adına atıf: çöz
  while (tip && ts.isTypeReferenceNode(tip) && tipDizini.has(tip.typeName.getText(tipler))) tip = tipDizini.get(tip.typeName.getText(tipler));
  if (!tip || !ts.isTypeLiteralNode(tip)) return null;
  const alanlar = {};
  for (const u of tip.members) {
    if (!ts.isPropertySignature(u) || !u.name) continue;
    const ad = u.name.getText(tipler).replace(/^['"]|['"]$/g, '');
    alanlar[ad] = { zorunlu: !u.questionToken, kurallar: [] };
  }
  return alanlar;
};
const veriUyesi = (veriTipi, ad) => {
  const t = tipDizini.get(veriTipi);
  if (!t || !ts.isTypeLiteralNode(t)) return null;
  const u = t.members.find((m) => ts.isPropertySignature(m) && m.name.getText(tipler) === ad);
  if (!u || !u.type || u.type.kind === ts.SyntaxKind.NeverKeyword) return null;
  return uyeler(u.type);
};

const endpointler = [];
const gez = (n) => {
  // (options.client ?? client).post<Responses, Errors, ThrowOnError>({ url: '/x', security: [...] , ... })
  if (ts.isCallExpression(n) && ts.isPropertyAccessExpression(n.expression) && n.arguments[0] && ts.isObjectLiteralExpression(n.arguments[0])) {
    const yontem = n.expression.name.text.toUpperCase();
    const ozellik = (ad) => n.arguments[0].properties.find((p) => p.name && p.name.getText(sdk) === ad);
    const url = ozellik('url');
    if (url && ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS', 'HEAD'].includes(yontem)) {
      const yol = url.initializer.text.replace(/\{([^}]+)\}/g, ':$1');
      const giris = ozellik('security') ? 'gerekli' : 'herkese_acik';
      // Veri tipi: (options: Options<xxxData, ...>) parametresinden
      let fn = n;
      while (fn && !ts.isMethodDeclaration(fn)) fn = fn.parent;
      const p = fn && fn.parameters[0];
      const tipArg = p && p.type && ts.isTypeReferenceNode(p.type) && p.type.typeArguments && p.type.typeArguments[0];
      const veri = tipArg ? tipArg.getText(sdk) : null;
      const govde = veri ? veriUyesi(veri, 'body') : null;
      const sorgu = veri ? veriUyesi(veri, 'query') : null;
      endpointler.push({
        id: `${yontem} ${yol}`,
        yontem,
        yol,
        giris,
        roller: [],
        govde: govde ? [{ sema: { alanlar: govde } }] : [],
        sorgu: sorgu ? [{ sema: { alanlar: sorgu } }] : [],
      });
    }
  }
  ts.forEachChild(n, gez);
};
gez(sdk);
fs.writeFileSync(cikti, JSON.stringify({ kaynak: 'hey-api sdk.gen.ts + types.gen.ts (uygulamanın OpenAPI belgesinden üretilmiş)', endpointler }, null, 2));
console.log(`${endpointler.length} endpoint`);
