// Sets the theme before first paint; mirrors mode-watcher (key "mode-watcher-mode": light | dark | system).
// External file: extension pages forbid inline scripts.
(function () {
  try {
    var m = localStorage.getItem("mode-watcher-mode");
    var dark = m === "dark" || (m !== "light" && matchMedia("(prefers-color-scheme: dark)").matches);
    var r = document.documentElement;
    r.classList.toggle("dark", dark);
    r.style.colorScheme = dark ? "dark" : "light";
  } catch (e) {}
})();
