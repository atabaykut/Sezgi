// İstemci çağrıları rota değildir
const api = axios.create({ baseURL: '/api' });
export const kitaplariGetir = () => api.get('/kitaplar');
export const kitapEkle = (k) => api.post('/kitaplar', k);
