// Ön kayıt 3 ölçümü: işlem tool'ları (A) ile arama düzeni (B), Immich test kopyasında gerçek yazma.
// Kullanım: node islem-araclari-olc.js <sezgi> <harita> <immich-url> <oturum.json> <cikti.jsonl> [tekrar=2]
// oturum.json: Immich girişinin cevabı ({ accessToken }); yönetici hesabıyla durum sıfırlanır ve sonuç denetlenir.
// Her çalıştırmada yeni bir Sezgi token'ı üretilir ve sonra iptal edilir.
"use strict";
const { execFileSync, spawnSync } = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");

const [sezgi, harita, url, oturumDosya, cikti, tekrarArg] = process.argv.slice(2);
const TEKRAR = Number(tekrarArg || 2);
const anahtar = JSON.parse(fs.readFileSync(oturumDosya, "utf8")).accessToken;
const IZINLER = [
  "album.get-all-albums:oku", "album.create-album:yaz", "album.get-album-info:oku", "album.update-album-info:yaz",
  "tag.get-all-tags:oku", "tag.create-tag:yaz", "shared-link.get-all-shared-links:oku", "shared-link.create-shared-link:yaz",
  "api-key.get-api-keys:oku", "api-key.create-api-key:yaz", "user.get-my-preferences:oku", "user.update-my-preferences:yaz",
];

// Bağlantı kopması (sunucunun kapattığı bağlantının yeniden kullanılması) birkaç kez yeniden denenir
async function api(yontem, yol, govde) {
  for (let deneme = 1; ; deneme++) {
    try {
      const c = await fetch(url + yol, { method: yontem, headers: { Authorization: `Bearer ${anahtar}`, "Content-Type": "application/json", Connection: "close" }, body: govde ? JSON.stringify(govde) : undefined });
      const t = await c.text();
      return t ? JSON.parse(t) : null;
    } catch (e) {
      if (deneme >= 5) throw e;
      await new Promise(r => setTimeout(r, 1000 * deneme));
    }
  }
}

let varsayilanTercihler = null;
async function sifirla() {
  for (const a of await api("GET", "/api/albums")) await api("DELETE", `/api/albums/${a.id}`);
  for (const t of await api("GET", "/api/tags")) await api("DELETE", `/api/tags/${t.id}`);
  for (const l of await api("GET", "/api/shared-links")) await api("DELETE", `/api/shared-links/${l.id}`);
  for (const k of await api("GET", "/api/api-keys")) await api("DELETE", `/api/api-keys/${k.id}`);
  await api("PUT", "/api/users/me/preferences", { memories: { enabled: true } });
  varsayilanTercihler = await api("GET", "/api/users/me/preferences");
}

const GOREVLER = [
  { id: "G1", istem: "'Yaz 2026' adlı, açıklaması 'Deniz tatili' olan bir albüm oluştur.",
    denetle: async () => (await api("GET", "/api/albums")).some(a => a.albumName === "Yaz 2026" && a.description === "Deniz tatili") },
  { id: "G2", istem: "'Yaz 2026' albümünün adını 'Yaz 2026 - Ege' yap.",
    kur: async () => api("POST", "/api/albums", { albumName: "Yaz 2026" }),
    denetle: async () => { const l = await api("GET", "/api/albums"); return l.length === 1 && l[0].albumName === "Yaz 2026 - Ege"; } },
  { id: "G3", istem: "'Seyahat' adlı bir etiket oluştur.",
    denetle: async () => (await api("GET", "/api/tags")).some(t => t.name === "Seyahat") },
  { id: "G4", istem: "'Yaz 2026 - Ege' albümü için indirmeye izin vermeyen bir paylaşım bağlantısı oluştur.",
    kur: async () => api("POST", "/api/albums", { albumName: "Yaz 2026 - Ege" }),
    denetle: async () => { const a = (await api("GET", "/api/albums"))[0]; return (await api("GET", "/api/shared-links")).some(l => l.album && l.album.id === a.id && l.allowDownload === false); } },
  { id: "G5", istem: "Adı 'yedekleme' olan ve yalnızca albümleri okuma izni (album.read) taşıyan bir API anahtarı oluştur.",
    denetle: async () => (await api("GET", "/api/api-keys")).some(k => k.name === "yedekleme" && JSON.stringify(k.permissions) === JSON.stringify(["album.read"])) },
  { id: "G6", istem: "Hesabımda Anılar (memories) özelliğini kapat.",
    denetle: async () => {
      const p = await api("GET", "/api/users/me/preferences");
      const digerleri = Object.keys(p).filter(k => k !== "memories").every(k => JSON.stringify(p[k]) === JSON.stringify(varsayilanTercihler[k]));
      return p.memories.enabled === false && digerleri;
    } },
];

function calistir(gorev, duzen, n) {
  const ad = `olc-${gorev.id}-${duzen}-${n}-${Date.now()}`;
  const args = ["token", "olustur", "--ad", ad, "--sure", "1", "--harita", harita];
  for (const i of IZINLER) args.push("--izin", i);
  const token = execFileSync(sezgi, args).toString().match(/szg_[0-9a-f]+/)[0];
  const klasor = fs.mkdtempSync(path.join(os.tmpdir(), "sezgi-olc-"));
  const env = { SEZGI_TOKEN: token };
  if (duzen === "B") env.SEZGI_ISLEM_ESIGI = "0";
  fs.writeFileSync(path.join(klasor, "mcp.json"), JSON.stringify({ mcpServers: { sezgi: { command: sezgi, args: ["mcp", harita], env } } }));
  const t0 = Date.now();
  // İstem standart girdiden verilir: Windows'ta kabuk üzerinden argüman olarak geçen tırnaklı metin bozulur
  const r = spawnSync("claude", ["-p", "--mcp-config", "mcp.json", "--strict-mcp-config", "--allowedTools", "mcp__sezgi__*", "--output-format", "stream-json", "--verbose"],
    { cwd: klasor, encoding: "utf8", input: gorev.istem, timeout: 600000, shell: process.platform === "win32", maxBuffer: 1 << 28 });
  const sure = (Date.now() - t0) / 1000;
  execFileSync(sezgi, ["token", "iptal", ad, "--harita", harita]);
  const olaylar = (r.stdout || "").split("\n").map(l => { try { return JSON.parse(l); } catch (_) { return null; } }).filter(Boolean);
  const sonuc = olaylar.find(o => o.type === "result") || {};
  const model = (olaylar.find(o => o.type === "system" && o.model) || {}).model || "";
  const cagrilar = olaylar.filter(o => o.type === "assistant").flatMap(o => (o.message.content || []).filter(c => c.type === "tool_use" && c.name.startsWith("mcp__sezgi__")).map(c => c.name.replace("mcp__sezgi__", "")));
  fs.rmSync(klasor, { recursive: true, force: true });
  return { sure, model, sonuc, cagrilar };
}

(async () => {
  for (let n = 1; n <= TEKRAR; n++) {
    for (const g of GOREVLER) {
      for (const duzen of ["A", "B"]) {
        await sifirla();
        if (g.kur) await g.kur();
        const { sure, model, sonuc, cagrilar } = calistir(g, duzen, n);
        const basari = await g.denetle();
        const u = sonuc.usage || {};
        const satir = { gorev: g.id, duzen, tekrar: n, basari, tur: sonuc.num_turns, maliyet: sonuc.total_cost_usd, giris: (u.input_tokens || 0) + (u.cache_read_input_tokens || 0) + (u.cache_creation_input_tokens || 0), cikis: u.output_tokens, sezgi_cagri: cagrilar.length, cagrilar, sure, model, son_mesaj: String(sonuc.result || "").replace(/[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g, "<e-posta>").replace(/[A-Za-z0-9+/_-]{40,}={0,2}/g, "<anahtar>").slice(0, 400) };
        fs.appendFileSync(cikti, JSON.stringify(satir) + "\n");
        console.log(`${g.id} ${duzen} #${n}: ${basari ? "BAŞARILI" : "BAŞARISIZ"} · tur ${satir.tur} · $${(satir.maliyet || 0).toFixed(3)} · çağrı ${cagrilar.join(",")}`);
      }
    }
  }
  await sifirla();
})();
