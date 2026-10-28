function protect(req, res, next) {
  if (!req.headers.authorization) return res.status(401).json({ message: 'Not authorized' });
  next();
}
function admin(req, res, next) {
  if (req.user && req.user.isAdmin) return next();
  return res.status(401).json({ message: 'Not authorized as an admin' });
}
module.exports = { protect, admin };
