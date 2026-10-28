const logoutUser = (req, res) => { res.status(200).json({ message: 'Logged out' }); };
const authUser = async (req, res) => { res.json({}); };
export { logoutUser, authUser };
