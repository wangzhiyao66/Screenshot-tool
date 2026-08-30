import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const HANDLE = 8; // 手柄命中半径（CSS px）

export async function mount(root, params) {
  const monitorId = Number(params.get("m") || 0);

  root.innerHTML = `
    <div class="ov-root">
      <canvas class="ov-canvas"></canvas>
      <div class="ov-tip" style="display:none"></div>
      <div class="ov-bar"></div>
      <div class="ov-toggle" id="ovToggle">窗口识别：开</div>
      <div class="ov-hint">拖拽框选 · 悬停自动识别窗口 · 单击锁定 · 方向键微调 · Enter 完成 · Esc 取消 · W 切换识别</div>
    </div>`;

  const canvas = root.querySelector(".ov-canvas");
  const ctx = canvas.getContext("2d");
  const tip = root.querySelector(".ov-tip");
  const bar = root.querySelector(".ov-bar");
  const hint = root.querySelector(".ov-hint");
  const toggle = root.querySelector("#ovToggle");

  let img = null;
  let imgPath = "";
  let k = 1;              // 图片物理像素 / 窗口 CSS 像素
  let sel = null;         // {x, y, w, h}，CSS px
  let mode = "idle";      // idle | draw | move | resize
  let resizeDir = "";
  let dragStart = null;
  let mouse = { x: -1, y: -1 };
  let dpr = window.devicePixelRatio || 1;
  let picking = null;
  let loadToken = 0;

  // 窗口/控件智能识别状态
  let monitorX = 0, monitorY = 0;          // 本显示器原点（全局逻辑点，相对主屏左上角）
  let smartDetect = true;                   // 是否启用悬停识别
  let smartLevel = "control";               // window | control
  let hoverRect = null;                     // 悬停命中区域（覆盖层本地 CSS px）
  let hoverLabel = "";
  let detectBusy = false;
  let lastDetect = 0;

  /* ---------- 初始化：事件驱动，同时用命令兜底防止事件漏接 ---------- */
  const unlisten = await listen("overlay://init", (e) => onInit(e.payload));
  window.addEventListener("beforeunload", () => unlisten());

  const cached = await invoke("get_overlay_init", { monitorId }).catch(() => null);
  if (cached) await onInit(cached);

  /* ---------- 事件绑定 ---------- */
  window.addEventListener("resize", () => { resizeCanvas(); draw(); });
  canvas.addEventListener("mousedown", onDown);
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
  canvas.addEventListener("dblclick", () => sel && finish("editor"));
  canvas.addEventListener("contextmenu", (e) => { e.preventDefault(); cancel(); });
  window.addEventListener("keydown", onKey);
  toggle.addEventListener("click", (e) => { e.stopPropagation(); toggleSmart(); });

  return;

  /* ==================== 初始化 ==================== */
  async function onInit(payload) {
    if (!payload || Number(payload.id) !== monitorId) return;
    const token = ++loadToken;
    imgPath = payload.path;
    try {
      await loadImage(payload.path);
    } catch (e) {
      toast(String(e));
      return;
    }
    if (token !== loadToken) return;

    dpr = window.devicePixelRatio || 1;
    resizeCanvas();
    // 窗口按物理像素铺满整个显示器，因此图片像素 / 窗口 CSS 宽度 就是换算系数
    k = window.innerWidth > 0 ? img.naturalWidth / window.innerWidth : dpr;
    if (!Number.isFinite(k) || k <= 0) k = dpr || 1;

    // 智能识别：记录显示器原点（物理像素 / scale = 全局逻辑点）与开关
    monitorX = (payload.x || 0) / (payload.scale || 1);
    monitorY = (payload.y || 0) / (payload.scale || 1);
    smartDetect = !!payload.smartDetect;
    smartLevel = payload.smartDetectLevel || "control";
    hoverRect = null;
    toggle.textContent = "窗口识别：" + (smartDetect ? "开" : "关");
    toggle.classList.toggle("off", !smartDetect);

    if (payload.fixed && payload.fixed.w > 4 && payload.fixed.h > 4) {
      sel = clampRect({
        x: payload.fixed.x / k,
        y: payload.fixed.y / k,
        w: payload.fixed.w / k,
        h: payload.fixed.h / k,
      });
      showBar();
    } else {
      sel = null;
      hideBar();
    }
    mode = "idle";
    draw();
  }

  function loadImage(path) {
    return new Promise((resolve, reject) => {
      let settled = false;
      const finish = (im) => { if (!settled) { settled = true; img = im; resolve(); } };
      const fail = (e) => { if (!settled) reject(e); };
      // 直接走 base64，避免 Tauri 2 的 asset 协议在部分 macOS 上返回 NULL 导致 WebKit 崩溃（CFRelease called with NULL）
      invoke("read_png_base64", { path })
        .then((b64) => {
          const im = new Image();
          im.onload = () => finish(im);
          im.onerror = () => fail(new Error("快照解码失败"));
          im.src = `data:image/png;base64,${b64}`;
        })
        .catch((e) => fail(e instanceof Error ? e : new Error(String(e))));
      setTimeout(() => { if (!settled) fail(new Error("快照加载超时")); }, 8000);
    });
  }

  /* ==================== 坐标与几何 ==================== */
  function resizeCanvas() {
    const w = window.innerWidth, h = window.innerHeight;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    canvas.style.width = w + "px";
    canvas.style.height = h + "px";
  }

  function toPhysical(r) {
    return {
      x: Math.round(r.x * k),
      y: Math.round(r.y * k),
      w: Math.round(r.w * k),
      h: Math.round(r.h * k),
    };
  }

  function clampRect(r) {
    const w = window.innerWidth, h = window.innerHeight;
    let x = r.x, y = r.y, rw = r.w, rh = r.h;
    if (rw < 0) { x = r.x + r.w; rw = -r.w; }
    if (rh < 0) { y = r.y + r.h; rh = -r.h; }
    x = Math.max(0, Math.min(x, w - 1));
    y = Math.max(0, Math.min(y, h - 1));
    return { x, y, w: Math.max(1, Math.min(rw, w - x)), h: Math.max(1, Math.min(rh, h - y)) };
  }

  function hitHandle(mx, my) {
    if (!sel) return "";
    const { x, y, w, h } = sel;
    const pts = {
      nw: [x, y], n: [x + w / 2, y], ne: [x + w, y],
      e: [x + w, y + h / 2], se: [x + w, y + h], s: [x + w / 2, y + h],
      sw: [x, y + h], w: [x, y + h / 2],
    };
    for (const key of Object.keys(pts)) {
      const [px, py] = pts[key];
      if (Math.abs(mx - px) <= HANDLE && Math.abs(my - py) <= HANDLE) return key;
    }
    return "";
  }

  function insideSel(mx, my) {
    return !!sel && mx > sel.x && mx < sel.x + sel.w && my > sel.y && my < sel.y + sel.h;
  }

  /* ==================== 交互 ==================== */
  function onDown(e) {
    if (e.button !== 0 || !img) return;
    const mx = e.clientX, my = e.clientY;
    const dir = hitHandle(mx, my);
    if (dir) { mode = "resize"; resizeDir = dir; dragStart = { mx, my, sel: { ...sel } }; return; }
    if (insideSel(mx, my)) { mode = "move"; dragStart = { mx, my, sel: { ...sel } }; return; }

    // 智能识别：悬停命中窗口/控件时，单击直接锁定该区域（按住 Alt 可强制手动框选）
    if (smartDetect && hoverRect && !e.altKey) {
      sel = { ...hoverRect };
      hoverRect = null;
      hideBar();
      draw();
      showBar();
      return;
    }

    mode = "draw";
    dragStart = { mx, my };
    sel = { x: mx, y: my, w: 0, h: 0 };
    hoverRect = null;
    hideBar();
    draw();
  }

  function onMove(e) {
    mouse = { x: e.clientX, y: e.clientY };
    if (mode === "draw") {
      sel = clampRect({ x: dragStart.mx, y: dragStart.my, w: mouse.x - dragStart.mx, h: mouse.y - dragStart.my });
    } else if (mode === "move") {
      sel = clampRect({
        x: dragStart.sel.x + (mouse.x - dragStart.mx),
        y: dragStart.sel.y + (mouse.y - dragStart.my),
        w: dragStart.sel.w, h: dragStart.sel.h,
      });
    } else if (mode === "resize") {
      sel = clampRect(resizeBy(resizeDir, dragStart.sel, mouse.x - dragStart.mx, mouse.y - dragStart.my));
    } else {
      canvas.style.cursor = hoverRect ? "pointer"
        : hitHandle(mouse.x, mouse.y) ? "nwse-resize"
        : insideSel(mouse.x, mouse.y) ? "move" : "crosshair";
      maybeDetect();
    }
    if (!picking) picking = setTimeout(() => { picking = null; updateColor(); }, 60);
    draw();
  }

  function resizeBy(dir, s0, dx, dy) {
    let { x, y, w, h } = s0;
    if (dir.includes("w")) { x = s0.x + dx; w = s0.w - dx; }
    if (dir.includes("e")) { w = s0.w + dx; }
    if (dir.includes("n")) { y = s0.y + dy; h = s0.h - dy; }
    if (dir.includes("s")) { h = s0.h + dy; }
    return { x, y, w, h };
  }

  function onUp() {
    if (mode === "draw" && sel && (sel.w < 3 || sel.h < 3)) { sel = null; hoverRect = null; draw(); }
    if (mode !== "idle" && sel) showBar();
    mode = "idle";
    draw();
  }

  function onKey(e) {
    if (e.key === "Escape") { e.preventDefault(); cancel(); return; }
    if (e.key === "Enter" && sel) { e.preventDefault(); finish("editor"); return; }
    if (e.key === "w" || e.key === "W") { e.preventDefault(); toggleSmart(); return; }
    if (!sel) return;
    const step = e.shiftKey ? 10 : 1;
    const map = {
      ArrowLeft: [-step, 0], ArrowRight: [step, 0],
      ArrowUp: [0, -step], ArrowDown: [0, step],
    };
    if (map[e.key]) {
      e.preventDefault();
      const [dx, dy] = map[e.key];
      sel = clampRect({ ...sel, x: sel.x + dx, y: sel.y + dy });
      showBar(); draw();
    }
  }

  /* ==================== 智能识别 ==================== */
  function maybeDetect() {
    if (!smartDetect || !img || sel || mode !== "idle") { hoverRect = null; return; }
    const now = performance.now();
    if (detectBusy || now - lastDetect < 45) return;
    lastDetect = now;
    detectBusy = true;
    const gx = monitorX + mouse.x;
    const gy = monitorY + mouse.y;
    invoke("detect_element", { x: gx, y: gy, level: smartLevel })
      .then((r) => {
        if (mode !== "idle" || sel || !smartDetect) { hoverRect = null; return; }
        if (r && isFinite(r.w) && r.w > 2 && r.h > 2) {
          hoverRect = { x: r.x - monitorX, y: r.y - monitorY, w: r.w, h: r.h };
          hoverLabel = (r.title || r.role || "").trim();
        } else {
          hoverRect = null;
        }
      })
      .catch(() => { hoverRect = null; })
      .finally(() => {
        detectBusy = false;
        if (mode === "idle" && !sel) draw();
      });
  }

  function toggleSmart() {
    smartDetect = !smartDetect;
    toggle.textContent = "窗口识别：" + (smartDetect ? "开" : "关");
    toggle.classList.toggle("off", !smartDetect);
    if (!smartDetect) hoverRect = null;
    draw();
  }

  async function updateColor() {
    if (!img || mouse.x < 0) return;
    try {
      const p = toPhysical({ x: mouse.x, y: mouse.y, w: 0, h: 0 });
      const hex = await invoke("pick_color", { path: imgPath, x: p.x, y: p.y });
      tip.textContent = sel ? `${toPhysical(sel).w} × ${toPhysical(sel).h}　${hex}` : hex;
    } catch { /* 取色失败忽略 */ }
  }

  /* ==================== 绘制 ==================== */
  function draw() {
    const w = window.innerWidth, h = window.innerHeight;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    if (img) ctx.drawImage(img, 0, 0, w, h);

    ctx.fillStyle = "rgba(12, 16, 20, 0.42)";
    ctx.fillRect(0, 0, w, h);

    // 智能识别：悬停高亮窗口/控件（聚光 + 蓝色描边）
    if (hoverRect && !sel) {
      const { x, y, w: sw, h: sh } = hoverRect;
      if (img) {
        ctx.save();
        ctx.beginPath();
        ctx.rect(x, y, sw, sh);
        ctx.clip();
        ctx.drawImage(img, 0, 0, w, h);
        ctx.restore();
      }
      ctx.strokeStyle = "rgba(47, 128, 255, 0.35)";
      ctx.lineWidth = 4;
      ctx.strokeRect(x, y, sw, sh);
      ctx.strokeStyle = "#2f80ff";
      ctx.lineWidth = 2;
      ctx.strokeRect(x + 1, y + 1, sw - 2, sh - 2);
      ctx.lineWidth = 1;
    }

    if (sel) {
      const { x, y, w: sw, h: sh } = sel;
      if (img) {
        ctx.save();
        ctx.beginPath();
        ctx.rect(x, y, sw, sh);
        ctx.clip();
        ctx.drawImage(img, 0, 0, w, h);
        ctx.restore();
      }
      ctx.strokeStyle = "#ffffff";
      ctx.lineWidth = 1;
      ctx.strokeRect(x + 0.5, y + 0.5, sw - 1, sh - 1);
      ctx.strokeStyle = "rgba(0,0,0,.35)";
      ctx.strokeRect(x - 0.5, y - 0.5, sw + 1, sh + 1);

      ctx.strokeStyle = "rgba(255,255,255,.28)";
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      for (let i = 1; i < 3; i++) {
        ctx.moveTo(x + (sw * i) / 3, y); ctx.lineTo(x + (sw * i) / 3, y + sh);
        ctx.moveTo(x, y + (sh * i) / 3); ctx.lineTo(x + sw, y + (sh * i) / 3);
      }
      ctx.stroke();
      ctx.setLineDash([]);

      const hs = 7;
      ctx.fillStyle = "#fff";
      ctx.strokeStyle = "#d85a30";
      ctx.lineWidth = 1;
      for (const [hx, hy] of [[x, y], [x + sw / 2, y], [x + sw, y], [x + sw, y + sh / 2],
        [x + sw, y + sh], [x + sw / 2, y + sh], [x, y + sh], [x, y + sh / 2]]) {
        ctx.beginPath();
        ctx.rect(Math.round(hx) - hs / 2, Math.round(hy) - hs / 2, hs, hs);
        ctx.fill(); ctx.stroke();
      }

      const ph = toPhysical(sel);
      tip.style.display = "block";
      tip.style.left = Math.min(x, w - 180) + "px";
      tip.style.top = (y > 26 ? y - 24 : Math.min(y + sh + 6, h - 26)) + "px";
      const parts = tip.textContent.split("　");
      tip.textContent = `${ph.w} × ${ph.h}` + (parts[1] ? `　${parts[1]}` : "");
    } else {
      tip.style.display = "block";
      if (hoverRect) {
        tip.style.left = Math.min(hoverRect.x, w - 240) + "px";
        tip.style.top = (hoverRect.y > 26
          ? hoverRect.y - 24
          : Math.min(hoverRect.y + hoverRect.h + 6, h - 30)) + "px";
        tip.textContent = (hoverLabel ? hoverLabel + "  " : "") +
          `${Math.round(hoverRect.w)} × ${Math.round(hoverRect.h)}`;
      } else {
        tip.style.left = Math.min(mouse.x + 14, w - 90) + "px";
        tip.style.top = Math.min(mouse.y + 18, h - 30) + "px";
        tip.textContent = `${mouse.x}, ${mouse.y}`;
      }
    }

    if ((mode === "draw" || mode === "resize") && img && mouse.x >= 0) drawMagnifier();
    if (sel) positionBar();
    hint.style.display = sel ? "none" : "block";
  }

  function drawMagnifier() {
    const srcW = 34, srcH = 22, zoom = 7;
    const sx = mouse.x * k - srcW / 2;
    const sy = mouse.y * k - srcH / 2;
    const dw = srcW * zoom, dh = srcH * zoom;
    let dx = mouse.x + 20, dy = mouse.y + 20;
    if (dx + dw > window.innerWidth - 8) dx = mouse.x - dw - 20;
    if (dy + dh > window.innerHeight - 8) dy = mouse.y - dh - 20;

    ctx.save();
    ctx.imageSmoothingEnabled = false;
    ctx.beginPath();
    ctx.rect(dx, dy, dw, dh);
    ctx.clip();
    ctx.drawImage(img, sx, sy, srcW, srcH, dx, dy, dw, dh);
    ctx.restore();

    ctx.strokeStyle = "rgba(255,255,255,.9)";
    ctx.lineWidth = 2;
    ctx.strokeRect(dx, dy, dw, dh);
    ctx.strokeStyle = "rgba(216,90,48,.9)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(dx + dw / 2, dy + dh / 2 - 9); ctx.lineTo(dx + dw / 2, dy + dh / 2 + 9);
    ctx.moveTo(dx + dw / 2 - 9, dy + dh / 2); ctx.lineTo(dx + dw / 2 + 9, dy + dh / 2);
    ctx.stroke();
  }

  /* ==================== 工具栏 ==================== */
  function buildBar() {
    const items = [
      ["标注", () => finish("editor")],
      ["识别文字", () => finish("editor", true)],
      ["sep", null],
      ["复制", () => finish("copy")],
      ["保存", () => finish("save")],
      ["固定", () => finish("pin")],
      ["sep", null],
      ["固定区域", () => fixRegion()],
      ["sep", null],
      ["取消", () => cancel()],
    ];
    bar.innerHTML = "";
    for (const [label, fn] of items) {
      if (label === "sep") {
        const s = document.createElement("div");
        s.className = "sep";
        bar.appendChild(s);
        continue;
      }
      const b = document.createElement("button");
      b.textContent = label;
      b.onclick = (e) => { e.stopPropagation(); fn(); };
      bar.appendChild(b);
    }
  }

  function positionBar() {
    const { x, y, w: sw, h: sh } = sel;
    const w = window.innerWidth, h = window.innerHeight;
    const bw = bar.offsetWidth || 340;
    let bx = x + sw / 2;
    bx = Math.max(bw / 2 + 8, Math.min(bx, w - bw / 2 - 8));
    let by = y + sh + 10;
    if (by + 44 > h) by = Math.max(8, y - 48);
    bar.style.left = bx + "px";
    bar.style.top = by + "px";
  }

  function showBar() { if (!bar.innerHTML) buildBar(); bar.classList.add("show"); positionBar(); }
  function hideBar() { bar.classList.remove("show"); }

  async function fixRegion() {
    const r = toPhysical(sel);
    try {
      await invoke("set_fixed_region", {
        monitorId, x: r.x, y: r.y, w: r.w, h: r.h,
      });
      toast(`已固定区域 ${r.w}×${r.h}，之后按「固定区域截图」快捷键可一键重截`);
    } catch (e) { toast(String(e)); }
  }

  /* ==================== 收尾 ==================== */
  async function finish(action, withOcr = false) {
    if (!sel) return;
    const r = toPhysical(sel);
    hideBar();
    try {
      const out = await invoke("crop_and_save", {
        path: imgPath, x: r.x, y: r.y, w: r.w, h: r.h,
      });
      sel = null;
      await invoke("close_overlays");
      if (action === "editor") await invoke("open_editor_cmd", { path: out.path, ocr: withOcr });
      else if (action === "copy") await invoke("copy_image_file", { path: out.path });
      else if (action === "save") await invoke("save_image_as", { path: out.path });
      else if (action === "pin") await invoke("open_pin_cmd", { path: out.path });
    } catch (e) {
      toast("操作失败：" + e);
    }
  }

  async function cancel() {
    sel = null;
    hoverRect = null;
    hideBar();
    try { await invoke("close_overlays"); } catch { /* noop */ }
  }

  function toast(msg) {
    const t = document.createElement("div");
    t.className = "toast";
    t.textContent = msg;
    document.body.appendChild(t);
    setTimeout(() => t.remove(), 2600);
  }
}
