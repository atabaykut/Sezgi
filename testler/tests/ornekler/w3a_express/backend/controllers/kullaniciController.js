import asyncHandler from '../middleware/asyncHandler.js';
import Kullanici from '../models/kullanici.js';

// @desc    Register a new user
const kayit = asyncHandler(async (req, res) => {
  const { ad, eposta, sifre } = req.body;
  // get the existing user from our database
  const var_mi = await Kullanici.findOne({ eposta });
  if (var_mi) {
    res.status(400);
    throw new Error('Kullanici zaten var');
  }
  res.status(201).json({ ad });
});

export { kayit };
