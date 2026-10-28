import { type RouteConfig, index, layout, prefix, route } from "@react-router/dev/routes";

export default [
  index("routes/home.tsx"),
  route("about", "routes/about.tsx"),
  layout("routes/auth/layout.tsx", [route("login", "routes/auth/login.tsx")]),
  ...prefix("concerts", [index("routes/concerts/home.tsx"), route(":city", "routes/concerts/city.tsx")]),
] satisfies RouteConfig;
