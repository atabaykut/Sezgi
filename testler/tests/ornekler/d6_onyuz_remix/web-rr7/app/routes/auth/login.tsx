import { Form } from "react-router";

export async function action({ request }: { request: Request }) {
  const form = await request.formData();
  await fetch("http://localhost:4000/api/notes", { method: "POST", body: form });
  return null;
}

export default function Login() {
  return (
    <Form method="post">
      <button type="submit">Giriş yap</button>
    </Form>
  );
}
