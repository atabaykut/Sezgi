import { A, useParams } from "@solidjs/router";

export default function User() {
  const params = useParams();
  const sil = async () => {
    await fetch(`/api/todos/${params.id}`, { method: "DELETE" });
  };
  return (
    <div>
      <button onClick={sil}>Remove</button>
      <A href="/">Ana sayfa</A>
    </div>
  );
}
