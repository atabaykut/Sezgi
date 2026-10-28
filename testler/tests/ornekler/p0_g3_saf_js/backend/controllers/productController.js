const getProducts = async (req, res) => {
  res.json([]);
};

const createProduct = async (req, res) => {
  const { name, price } = req.body;
  res.status(201).json({ name, price });
};

export { getProducts, createProduct };
