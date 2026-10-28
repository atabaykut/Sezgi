import { action, query, redirect } from "@solidjs/router";

const API = "http://localhost:5000/api";

export const getTodos = query(async () => {
  "use server";
  const yanit = await fetch(`${API}/todos`);
  return yanit.json();
}, "todos");

export const addTodo = action(async (form: FormData) => {
  "use server";
  await fetch(`${API}/todos`, { method: "POST", body: form });
  throw redirect("/todos");
});

export const removeTodo = action(async (id: string) => {
  "use server";
  await fetch(`${API}/todos/${id}`, { method: "DELETE" });
});
