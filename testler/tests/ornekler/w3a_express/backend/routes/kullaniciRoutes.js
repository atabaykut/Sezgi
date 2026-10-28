import express from 'express';
import { kayit } from '../controllers/kullaniciController.js';

const router = express.Router();
router.route('/').post(kayit);

export default router;
