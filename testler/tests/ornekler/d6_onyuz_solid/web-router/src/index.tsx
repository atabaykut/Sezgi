import { render } from "solid-js/web";
import { Route, Router } from "@solidjs/router";
import { lazy } from "solid-js";
import App from "./App";
import Home from "./pages/Home";

const Users = lazy(() => import("./pages/Users"));

render(
  () => (
    <Router root={App}>
      <Route path="/" component={Home} />
      <Route path="/users" component={Users} />
      <Route path="/users/:id" component={lazy(() => import("./pages/User"))} />
    </Router>
  ),
  document.getElementById("root")!,
);
