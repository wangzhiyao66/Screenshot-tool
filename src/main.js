import "./styles.css";

const params = new URLSearchParams(location.search);
const win = params.get("win") || "settings";

const ROUTES = {
  overlay: () => import("./overlay.js"),
  editor: () => import("./editor.js"),
  pin: () => import("./pin.js"),
  settings: () => import("./settings.js"),
};

document.body.classList.add(`win-${win}`);

(ROUTES[win] || ROUTES.settings)()
  .then((m) => m.mount(document.getElementById("app"), params))
  .catch((e) => {
    document.body.innerHTML =
      `<pre style="padding:20px;color:#a32d2d;white-space:pre-wrap">${e?.stack || e}</pre>`;
  });
