import { TOKEN_KEY } from "./settings";

const input = document.getElementById("token") as HTMLInputElement;
const status = document.getElementById("status") as HTMLElement;

chrome.storage.local.get(TOKEN_KEY).then((v) => {
  const t = v[TOKEN_KEY];
  if (typeof t === "string") input.value = t;
});

document.getElementById("save")?.addEventListener("click", async () => {
  const token = input.value.trim();
  if (token) await chrome.storage.local.set({ [TOKEN_KEY]: token });
  else await chrome.storage.local.remove(TOKEN_KEY);
  status.textContent = "Saved";
  setTimeout(() => (status.textContent = ""), 1500);
});
