import express from 'express';
const router = express.Router();
import { logoutUser, authUser } from '../controllers/userController.js';

router.post('/logout', logoutUser);
router.post('/auth', authUser);

export default router;
