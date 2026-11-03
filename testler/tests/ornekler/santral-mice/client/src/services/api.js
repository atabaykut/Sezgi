import axios from 'axios';

const API_URL = process.env.REACT_APP_API_URL || '/mice/api';

const api = axios.create({ baseURL: API_URL });

export const cikis = () => api.post('/auth/logout');
export const unvanlar = () => api.get('/titles');
