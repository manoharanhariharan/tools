import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";
import { applyStoredTheme } from "./lib/theme";

applyStoredTheme();

const target = document.getElementById("app");
if (!target) throw new Error("missing #app root element");

const app = mount(App, { target });

export default app;
