import type { ActionFunctionArgs } from "@remix-run/node";
import { redirect } from "@remix-run/node";
import { Form } from "@remix-run/react";
import { API } from "~/config";

export async function action({ request }: ActionFunctionArgs) {
  const form = await request.formData();
  const yanit = await fetch(`${API}/notes`, { method: "POST", body: form });
  const not = await yanit.json();
  return redirect(`/notes/${not.id}`);
}

export default function NewNote() {
  return (
    <Form method="post">
      <input name="title" />
      <button>Create note</button>
    </Form>
  );
}
