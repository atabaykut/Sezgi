import type { ActionFunctionArgs } from "@remix-run/node";
import { json } from "@remix-run/node";
import { API } from "~/config";

export async function action({ request }: ActionFunctionArgs) {
  const form = await request.formData();
  await fetch(`${API}/notes/${form.get("id")}/like`, { method: "POST" });
  return json({ ok: true });
}
