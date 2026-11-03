import axios from 'axios';

const sabit = axios.create({ baseURL: '/mice/api' });
const ortam = axios.create({ baseURL: import.meta.env.VITE_API || '/mice/api' });

export const unvanlar = () => sabit.get('/titles');
export const cikis = () => ortam.post('/auth/logout');
export const sayfa = () => fetch('/sayfa');
