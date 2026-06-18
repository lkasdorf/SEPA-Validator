import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { get } from "svelte/store";
import { theme } from "./lib/stores";

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app

const mq = window.matchMedia("(prefers-color-scheme: dark)");
function apply(t: "system" | "light" | "dark") {
  const dark = t === "dark" || (t === "system" && mq.matches);
  document.documentElement.setAttribute("data-theme", dark ? "dark" : "light");
}
theme.subscribe(apply);
mq.addEventListener("change", () => apply(get(theme)));
