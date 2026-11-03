function authMiddleware(req, res, next) {
  if (!req.headers.authorization) {
    return res.status(401).json({ hata: 'Unauthorized' });
  }
  next();
}

// Kimlik varsa okur, yoksa isteği geçirir
function optionalAuth(req, res, next) {
  next();
}

function requireRole(...roller) {
  return (req, res, next) => next();
}

const asyncHandler = (fn) => (req, res, next) => Promise.resolve(fn(req, res, next)).catch(next);

module.exports = { authMiddleware, optionalAuth, requireRole, asyncHandler };
