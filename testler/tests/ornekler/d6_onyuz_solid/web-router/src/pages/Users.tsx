import { createResource, For } from "solid-js";

const kullanicilariGetir = async () => (await fetch("/api/users/1")).json();

export default function Users() {
  const [liste] = createResource(kullanicilariGetir);
  return <For each={[liste()]}>{u => <p>{u?.name}</p>}</For>;
}
