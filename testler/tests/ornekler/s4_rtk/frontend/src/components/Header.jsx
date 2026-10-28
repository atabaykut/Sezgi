import { useNavigate } from 'react-router-dom';
import { useLogoutMutation } from '../slices/usersApiSlice';

const Header = () => {
  const navigate = useNavigate();
  const [logoutApiCall] = useLogoutMutation();

  const logoutHandler = async () => {
    try {
      await logoutApiCall().unwrap();
      navigate('/login');
    } catch (err) {
      console.error(err);
    }
  };

  return (
    <header>
      <button onClick={logoutHandler}>Logout</button>
    </header>
  );
};

export default Header;
