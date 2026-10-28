// Ön yüz: IPC köprüsü ve sunucuya istek
document.getElementById('iptal').addEventListener('click', async () => {
  const sonuc = await window.otel.iptal(42);
  window.otel.gunluk('iptal ' + sonuc.tamam);
});

document.getElementById('senkron').addEventListener('click', async () => {
  const liste = await window.otel.liste();
  await fetch('http://127.0.0.1:8000/api/v1/rezervasyonlar/esitle', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ adet: liste.length }),
  });
});
