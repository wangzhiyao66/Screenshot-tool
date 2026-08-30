import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export async function mount(root, params) {
  const path = params.get("src") || "";
  const natW = +params.get("w") || 0;
  const natH = +params.get("h") || 0;

  root.innerHTML = `
    <div class="pin-root" id="pr">
      <img class="pin-img" id="pi" draggable="false">
      <div class="pin-tools">
        <button id="zOut">−</button>
        <span class="zoom" id="zLabel">100%</span>
        <button id="zIn">+</button>
        <button id="zReset">1:1</button>
        <button id="rot">旋转</button>
        <button id="op">透明度</button>
        <button id="thru">穿透</button>
        <button id="cp">复制</button>
        <button id="sv">保存</button>
        <button id="cl" title="取消固定">✕</button>
      </div>
    </div>`;

  const pr = root.querySelector("#pr");
  const pi = root.querySelector("#pi");
  const zLabel = root.querySelector("#zLabel");
  const appWindow = getCurrentWindow();

  let scale = 1;          // 相对 1:1 逻辑像素
  let rot = 0;            // 0/90/180/270
  let opacity = 1;
  let thru = false;
  let dpr = window.devicePixelRatio || 1;

  // CSS 尺寸 = 物理像素 / dpr（1:1 显示）
  const baseW = natW ? natW / dpr : 0;
  const baseH = natH ? natH / dpr : 0;

  await new Promise((res, rej) => {
    let settled = false;
    // 直接走 base64，避免 Tauri 2 的 asset 协议在部分 macOS 上返回 NULL 导致 WebKit 崩溃
    invoke("read_png_base64", { path })
      .then((b64) => {
        pi.onload = () => { if (!settled) { settled = true; res(); } };
        pi.src = `data:image/png;base64,${b64}`;
      })
      .catch((e) => { if (!settled) rej(e instanceof Error ? e : new Error(String(e))); });
    setTimeout(() => { if (!settled) rej(new Error("固定加载超时")); }, 8000);
  });

  const iw = pi.naturalWidth || natW;
  const ih = pi.naturalHeight || natH;
  const cssW = baseW || iw / dpr;
  const cssH = baseH || ih / dpr;

  apply();
  fitToImage();

  /* ---------- 交互 ---------- */
  pr.addEventListener("mousedown", (e) => {
    if (e.target.closest(".pin-tools")) return;
    if (e.button === 0) appWindow.startDragging().catch(() => {});
  });

  // 缩放：滚轮；Ctrl/⌘ + 滚轮：透明度
  pr.addEventListener("wheel", (e) => {
    e.preventDefault();
    if (e.ctrlKey || e.metaKey) {
      opacity = clamp(opacity - e.deltaY / 1200, 0.15, 1);
    } else {
      const k = e.deltaY < 0 ? 1.12 : 1 / 1.12;
      setScale(scale * k);
    }
    apply();
  }, { passive: false });

  pr.addEventListener("dblclick", () => { scale = 1; rot = 0; opacity = 1; apply(); fitToImage(); });

  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") appWindow.close();
    else if (e.key === "+" || e.key === "=") { setScale(scale * 1.15); apply(); }
    else if (e.key === "-") { setScale(scale / 1.15); apply(); }
    else if (e.key === "0") { fitToImage(); scale = Math.min(scale, 1); apply(); }
  });

  root.querySelector("#zIn").onclick = () => { setScale(scale * 1.2); apply(); };
  root.querySelector("#zOut").onclick = () => { setScale(scale / 1.2); apply(); };
  root.querySelector("#zReset").onclick = async () => {
    scale = 1; rot = 0; opacity = 1; apply();
    await appWindow.setSize({ type: "Physical", width: Math.round(cssW * dpr), height: Math.round(cssH * dpr) });
  };
  root.querySelector("#rot").onclick = async () => {
    rot = (rot + 90) % 360; apply();
    const swap = rot % 180 !== 0;
    await appWindow.setSize({
      type: "Physical",
      width: Math.round((swap ? cssH : cssW) * scale * dpr),
      height: Math.round((swap ? cssW : cssH) * scale * dpr),
    });
  };
  root.querySelector("#op").onclick = () => {
    opacity = opacity > 0.85 ? 0.6 : opacity > 0.4 ? 0.25 : 1;
    apply();
  };
  root.querySelector("#thru").onclick = () => { thru = !thru; apply(); };
  root.querySelector("#cp").onclick = async () => {
    await invoke("copy_image_file", { path });
    toast("已复制到剪贴板");
  };
  root.querySelector("#sv").onclick = async () => {
    const r = await invoke("save_image_as", { path });
    if (r) toast("已保存：" + r);
  };
  root.querySelector("#cl").onclick = () => appWindow.close();

  function setScale(v) { scale = clamp(v, 0.1, 8); }

  function apply() {
    const swap = rot % 180 !== 0;
    pi.style.width = cssW * scale + "px";
    pi.style.height = cssH * scale + "px";
    pi.style.transform = `rotate(${rot}deg)`;
    pi.style.opacity = String(opacity);
    if (rot === 90) pi.style.transformOrigin = "0 0", pi.style.left = "100%", pi.style.top = "0";
    else if (rot === 180) pi.style.left = "100%", pi.style.top = "100%";
    else if (rot === 270) pi.style.left = "0", pi.style.top = "100%";
    else pi.style.left = "0", pi.style.top = "0";
    pr.style.transformOrigin = "0 0";
    zLabel.textContent = Math.round(scale * 100) + "%";
    root.querySelector("#thru").classList.toggle("on", thru);
    pr.classList.toggle("pin-through", thru);
    invoke("set_pin_click_through", { label: appWindow.label, through: thru }).catch(() => {});
    void swap;
  }

  async function fitToImage() {
    const w = Math.round(cssW * dpr), h = Math.round(cssH * dpr);
    const sw = await appWindow.currentMonitor().catch(() => null);
    const maxW = (sw?.size?.width || 1920) * 0.8;
    const maxH = (sw?.size?.height || 1080) * 0.8;
    if (w > maxW || h > maxH) {
      const k = Math.min(maxW / w, maxH / h);
      setScale(k);
      await appWindow.setSize({ type: "Physical", width: Math.round(w * k), height: Math.round(h * k) });
    } else {
      await appWindow.setSize({ type: "Physical", width: w, height: h });
    }
  }

  function toast(msg) {
    const t = document.createElement("div");
    t.className = "toast";
    t.textContent = msg;
    document.body.appendChild(t);
    setTimeout(() => t.remove(), 2000);
  }
}

function clamp(v, a, b) { return Math.max(a, Math.min(b, v)); }
