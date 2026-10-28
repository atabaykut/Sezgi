import type { ActionFunctionArgs, LoaderFunctionArgs } from "@remix-run/node";
import { json, redirect } from "@remix-run/node";
import { Form, useFetcher, useLoaderData } from "@remix-run/react";
import { API } from "~/config";

export async function loader({ params }: LoaderFunctionArgs) {
  const yanit = await fetch(`${API}/notes/${params.noteId}`);
  return json(await yanit.json());
}

export async function action({ params }: ActionFunctionArgs) {
  await fetch(`${API}/notes/${params.noteId}`, { method: "DELETE" });
  return redirect("/notes");
}

export default function Note() {
  const not = useLoaderData<typeof loader>();
  const fetcher = useFetcher();
  return (
    <div>
      <h2>{not.title}</h2>
      <Form method="post">
        <button type="submit">Delete</button>
      </Form>
      <button onClick={() => fetcher.submit({ id: not.id }, { method: "post", action: "/notes/like" })}>Like</button>
    </div>
  );
}
