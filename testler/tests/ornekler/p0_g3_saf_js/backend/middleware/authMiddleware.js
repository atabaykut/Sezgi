import jwt from 'jsonwebtoken';

const protect = (req, res, next) => {
  const token = req.cookies && req.cookies.jwt;
  if (!token) {
    return res.status(401).json({ message: 'Not authorized, no token' });
  }
  req.user = jwt.verify(token, process.env.JWT_SECRET);
  next();
};

const admin = (req, res, next) => {
  if (req.user && req.user.isAdmin) {
    return next();
  }
  return res.status(403).json({ message: 'Not authorized as admin' });
};

export { protect, admin };
