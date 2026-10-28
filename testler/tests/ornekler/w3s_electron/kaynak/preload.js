// Köprü: ön yüze yalnızca bu işlevler açılır
const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('otel', {
  iptal: (id) => ipcRenderer.invoke('rezervasyon:iptal', id),
  liste: () => ipcRenderer.invoke('rezervasyon:liste'),
  gunluk: (satir) => ipcRenderer.send('gunluk:yaz', satir),
});
