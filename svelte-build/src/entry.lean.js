import { mount } from "svelte";
import Search from "./SearchLean.svelte";
const el = document.getElementById("auohp-search");
if (el) mount(Search, { target: el });
