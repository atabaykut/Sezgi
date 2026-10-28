import { createAsync, useAction } from "@solidjs/router";
import { For } from "solid-js";
import { addTodo, getTodos, removeTodo } from "~/lib/api";

export default function Todos() {
  const todos = createAsync(() => getTodos());
  const sil = useAction(removeTodo);
  return (
    <div>
      <form action={addTodo} method="post">
        <input name="title" />
        <button type="submit">Add todo</button>
      </form>
      <For each={todos()}>
        {t => (
          <li>
            {t.title}
            <button onClick={() => sil(t.id)}>Delete</button>
          </li>
        )}
      </For>
    </div>
  );
}
