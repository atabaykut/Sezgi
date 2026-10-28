// Ana süreç: pencere ve IPC işleyicileri
const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');
const depo = require('./depo');

function pencereAc() {
  const pencere = new BrowserWindow({
    width: 900,
    height: 600,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });
  pencere.loadFile(path.join(__dirname, 'renderer', 'index.html'));
}

ipcMain.handle('rezervasyon:iptal', async (_olay, id) => {
  return depo.iptalEt(id);
});

ipcMain.handle('rezervasyon:liste', async () => depo.liste());

ipcMain.on('gunluk:yaz', (_olay, satir) => {
  depo.gunluk(satir);
});

app.whenReady().then(pencereAc);
