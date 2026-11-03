export enum Yollar {
  Kitap = 'kitaplar',
}
export const SURUM_2 = '2026-01-01';
export const SURUM_2_DEGERI = SURUM_2 as unknown as string;
export enum Tur {
  Roman = 'roman',
  Siir = 'siir',
}
// Değeri metin olmayan üye: değer listesi eksik kalacağı için hiç verilmemeli
export enum Oncelik {
  Dusuk,
  Yuksek = 'yuksek',
}
export const DILLER = ['tr', 'en', 'de'] as const;
// Ters tırnaklı ama değişkensiz değer de düz metindir
export enum Kategori {
  Bilim = `bilim`,
  Edebiyat = 'edebiyat',
}
