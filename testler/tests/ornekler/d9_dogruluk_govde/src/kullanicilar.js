import express from 'express';
import { kayit, guncelle } from './denetleyici.js';

const router = express.Router();

router.post('/', kayit);
router.put('/:id', guncelle);

export default router;
