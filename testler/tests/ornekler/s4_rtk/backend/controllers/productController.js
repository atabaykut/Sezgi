const getProducts = async (req, res) => { res.json([]); };
const getProductById = async (req, res) => { res.json({}); };
const createProduct = async (req, res) => { res.status(201).json({}); };
const updateProduct = async (req, res) => { res.json({}); };
const deleteProduct = async (req, res) => { res.json({ message: 'Product removed' }); };
export { getProducts, getProductById, createProduct, updateProduct, deleteProduct };
