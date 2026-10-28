import { Link, Outlet } from "@remix-run/react";

export default function App() {
  return (
    <html lang="tr">
      <body>
        <nav>
          <Link to="/notes">Notlar</Link>
        </nav>
        <Outlet />
      </body>
    </html>
  );
}
