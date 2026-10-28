import { A, useParams } from "@solidjs/router";
import { createResource } from "solid-js";

const API = "http://localhost:5000/api";

async function fetchUser(id: string) {
  const yanit = await fetch(`${API}/users/${id}`);
  return yanit.json();
}

export default function User() {
  const params = useParams();
  const [user] = createResource(() => params.id, fetchUser);
  return (
    <div>
      <p>{user()?.name}</p>
      <A href="/">Ana sayfa</A>
    </div>
  );
}
