import { A } from "@solidjs/router";

export default function App(props: { children?: unknown }) {
  return (
    <div>
      <A href="/users">Kullanıcılar</A>
      {props.children}
    </div>
  );
}
