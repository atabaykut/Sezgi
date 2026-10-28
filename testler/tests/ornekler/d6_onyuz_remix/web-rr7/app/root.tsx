import { Link, Outlet } from "react-router";

export default function Root() {
  return (
    <div>
      <Link to="/about">Hakkında</Link>
      <Outlet />
    </div>
  );
}
