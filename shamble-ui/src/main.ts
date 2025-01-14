import { invoke } from "@tauri-apps/api/core";

async function greet() {

  // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
  console.log(await invoke("greet", {

  }));

}

window.addEventListener("DOMContentLoaded", () => {
  console.log("Hi!")
});
