// Mount-to-div packaging: classic bundle, finds <div id="auohp-search">, mounts light DOM.
import { mount } from "svelte";
import Search from "./Search.svelte";

const el = document.getElementById("auohp-search");
if (el) mount(Search, { target: el });
