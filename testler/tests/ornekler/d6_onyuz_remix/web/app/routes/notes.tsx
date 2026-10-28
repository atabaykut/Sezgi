import { json } from "@remix-run/node";
import { Outlet, useLoaderData } from "@remix-run/react";
import { API } from "~/config";

export async function loader() {
  const yanit = await fetch(`${API}/notes`);
  return json(await yanit.json());
}

export default function NotesLayout() {
  const notlar = useLoaderData<typeof loader>();
  return (
    <div>
      <p>{notlar.length}</p>
      <Outlet />
    </div>
  );
}
