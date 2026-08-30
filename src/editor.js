import { invoke } from "@tauri-apps/api/core";

const COLORS = ["#e24b4a", "#ef9f27", "#639922", "#378add", "#1f2328", "#ffffff"];
const TOOLS = [
  ["rect", "矩形"], ["ellipse", "椭圆"], ["arrow", "箭头"], ["line", "直线"],
  ["pen", "画笔"], ["mosaic", "马赛克"], ["text", "文字"], ["number", "序号"],
];

export async function mount(root, params) {
  const src = params.get("src") || "";
  const autoOcr = params.get("ocr") === "1";

  root.innerHTML = `
    <div class="ed-root">
      <div class="ed-top">
        <div class="group" id="tools"></div>
        <div class="divider"></div>
        <div class="swatches" id="colors"></div>
        <div class="divider"></div>
        <div class="group" id="sizes"></div>
        <div class="spacer"></div>
        <div class="group">
          <button id="undo">撤销</button>
          <button id="redo">重做</button>
          <button id="clear">清空</button>
          <button id="fit">适应</button>
        </div>
        <div class="divider"></div>
        <div class="group">
          <button id="ocrBtn">识别文字</button>
          <button id="trBtn">翻译</button>
          <button id="pinBtn">贴图</button>
          <button id="copyBtn">复制</button>
          <button id="saveBtn">保存</button>
        </div>
      </div>
      <div class="ed-body">
        <div class="ed-canvas-wrap">
          <div class="ed-stage" id="stage">
            <canvas class="ed-canvas" id="cv"></canvas>
            <canvas class="ed-ovl" id="ovl"></canvas>
          </div>
        </div>
        <div class="ed-side" id="side">
          <header>
            <span class="t" id="sideTitle">识别结果</span>
            <button id="sideClose">关闭</button>
          </header>
          <div class="content" id="sideContent"></div>
          <div class="foot" id="sideFoot"></div>
        </div>
      </div>
    </div>`;

  const cv = root.querySelector("#cv");
  const ovl = root.querySelector("#ovl");
  const stage = root.querySelector("#stage");
  const ctx = cv.getContext("2d");
  const octx = ovl.getContext("2d");

  let img = null;
  let dpr = window.devicePixelRatio || 1;
  let viewScale = 1;       // 显示缩放（相对图片原始像素）
  let zoom = 1;            // 用户缩放
  let ops = [];
  let redoStack = [];
  let tool = "rect";
  let color = COLORS[0];
  let size = 3;
  let drawing = null;
  let ocrLines = [];

  /* ---------- 载入图片（base64，避免 canvas 被污染导致无法导出） ---------- */
  const b64 = await invoke("read_png_base64", { path: src });
  await new Promise((res, rej) => {
    img = new Image();
    img.onload = res;
    img.onerror = () => rej(new Error("图片加载失败"));
    img.src = `data:image/png;base64,${b64}`;
  });

  buildToolbar();
  layout();
  window.addEventListener("resize", layout);
  cv.addEventListener("mousedown", onDown);
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
  window.addEventListener("keydown", onKey);

  if (autoOcr) runOcr();

  /* ================= 工具栏 ================= */
  function buildToolbar() {
    const tg = root.querySelector("#tools");
    TOOLS.forEach(([id, label]) => {
      const b = document.createElement("button");
      b.textContent = label;
      b.dataset.tool = id;
      b.className = id === tool ? "on" : "";
      b.onclick = () => { tool = id; tg.querySelectorAll("button").forEach((x) => x.classList.toggle("on", x.dataset.tool === id)); };
      tg.appendChild(b);
    });

    const cg = root.querySelector("#colors");
    COLORS.forEach((c) => {
      const s = document.createElement("div");
      s.className = "swatch" + (c === color ? " on" : "");
      s.style.background = c;
      s.onclick = () => { color = c; cg.querySelectorAll(".swatch").forEach((x) => x.classList.remove("on")); s.classList.add("on"); };
      cg.appendChild(s);
    });

    const sg = root.querySelector("#sizes");
    [2, 3, 5, 9].forEach((n) => {
      const b = document.createElement("button");
      b.textContent = n + "px";
      b.className = n === size ? "on" : "";
      b.onclick = () => { size = n; sg.querySelectorAll("button").forEach((x) => x.classList.toggle("on", x.textContent === n + "px")); };
      sg.appendChild(b);
    });
  }

  /* ================= 布局 ================= */
  function layout() {
    const body = root.querySelector(".ed-body");
    const sideOn = root.querySelector("#side").classList.contains("show");
    const availW = body.clientWidth - (sideOn ? 300 : 0) - 48;
    const availH = body.clientHeight - 48;
    viewScale = Math.min(availW / img.naturalWidth, availH / img.naturalHeight, 1) * zoom;

    const dw = Math.round(img.naturalWidth * viewScale);
    const dh = Math.round(img.naturalHeight * viewScale);
    stage.style.width = dw + "px";
    stage.style.height = dh + "px";
    for (const c of [cv, ovl]) {
      c.width = Math.round(dw * dpr);
      c.height = Math.round(dh * dpr);
      c.style.width = dw + "px";
      c.style.height = dh + "px";
    }
    render();
    drawOcrOverlay();
  }

  /* ================= 绘制 ================= */
  function render() {
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cv.width, cv.height);
    ctx.save();
    ctx.scale(viewScale, viewScale);
    ctx.drawImage(img, 0, 0);
    for (const op of ops) drawOp(ctx, op);
    ctx.restore();
  }

  function drawOp(c, op) {
    c.save();
    c.strokeStyle = op.color;
    c.fillStyle = op.color;
    c.lineWidth = op.width;
    c.lineCap = "round";
    c.lineJoin = "round";
    const p = op.points;
    switch (op.type) {
      case "rect": c.strokeRect(p[0].x, p[0].y, p[1].x - p[0].x, p[1].y - p[0].y); break;
      case "ellipse": {
        const rx = (p[1].x - p[0].x) / 2, ry = (p[1].y - p[0].y) / 2;
        c.beginPath(); c.ellipse(p[0].x + rx, p[0].y + ry, Math.abs(rx), Math.abs(ry), 0, 0, Math.PI * 2); c.stroke(); break;
      }
      case "line": c.beginPath(); c.moveTo(p[0].x, p[0].y); c.lineTo(p[1].x, p[1].y); c.stroke(); break;
      case "arrow": {
        c.beginPath(); c.moveTo(p[0].x, p[0].y); c.lineTo(p[1].x, p[1].y); c.stroke();
        const a = Math.atan2(p[1].y - p[0].y, p[1].x - p[0].x), L = 10 + op.width * 2;
        c.beginPath();
        c.moveTo(p[1].x, p[1].y);
        c.lineTo(p[1].x - L * Math.cos(a - 0.42), p[1].y - L * Math.sin(a - 0.42));
        c.moveTo(p[1].x, p[1].y);
        c.lineTo(p[1].x - L * Math.cos(a + 0.42), p[1].y - L * Math.sin(a + 0.42));
        c.stroke(); break;
      }
      case "pen": {
        c.beginPath(); c.moveTo(p[0].x, p[0].y);
        for (let i = 1; i < p.length; i++) c.lineTo(p[i].x, p[i].y);
        c.stroke(); break;
      }
      case "mosaic": mosaic(c, p[0], p[1]); break;
      case "text":
        c.font = `${op.width * 6}px ${getComputedStyle(document.body).fontFamily}`;
        c.textBaseline = "top";
        c.fillText(op.text, p[0].x, p[0].y);
        break;
      case "number": {
        const r = 13 + op.width;
        c.beginPath(); c.arc(p[0].x, p[0].y, r, 0, Math.PI * 2); c.fill();
        c.fillStyle = "#fff";
        c.font = `600 ${Math.round(r * 1.15)}px ${getComputedStyle(document.body).fontFamily}`;
        c.textAlign = "center"; c.textBaseline = "middle";
        c.fillText(String(op.n), p[0].x, p[0].y + 1);
        break;
      }
    }
    c.restore();
  }

  function mosaic(c, a, b) {
    const x = Math.min(a.x, b.x), y = Math.min(a.y, b.y);
    const w = Math.abs(b.x - a.x), h = Math.abs(b.y - a.y);
    if (w < 2 || h < 2) return;
    const t = Math.max(3, Math.round(12 / viewScale));
    const tmp = document.createElement("canvas");
    tmp.width = Math.max(1, Math.round(w / 8));
    tmp.height = Math.max(1, Math.round(h / 8));
    const tc = tmp.getContext("2d");
    tc.imageSmoothingEnabled = false;
    tc.drawImage(img, x, y, w, h, 0, 0, tmp.width, tmp.height);
    c.imageSmoothingEnabled = false;
    c.drawImage(tmp, 0, 0, tmp.width, tmp.height, x, y, w, h);
    c.imageSmoothingEnabled = true;
    void t;
  }

  /* ================= 输入 ================= */
  function imgPt(e) {
    const r = cv.getBoundingClientRect();
    return { x: (e.clientX - r.left) / viewScale, y: (e.clientY - r.top) / viewScale };
  }

  function onDown(e) {
    if (e.button !== 0) return;
    const pt = imgPt(e);
    if (tool === "text") { askText(pt); return; }
    if (tool === "number") {
      const n = ops.filter((o) => o.type === "number").length + 1;
      ops.push({ type: "number", color, width: size, points: [pt], n });
      redoStack = []; render(); return;
    }
    drawing = { type: tool, color, width: size, points: [pt, pt] };
  }

  function onMove(e) {
    if (!drawing) return;
    if (drawing.type === "pen") drawing.points.push(imgPt(e));
    else drawing.points[1] = imgPt(e);
    render();
  }

  function onUp() {
    if (!drawing) return;
    const p = drawing.points;
    const moved = Math.hypot(p[1].x - p[0].x, p[1].y - p[0].y);
    if (drawing.type !== "pen" && moved < 2) { drawing = null; render(); return; }
    ops.push(drawing);
    redoStack = [];
    drawing = null;
    render();
  }

  function onKey(e) {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === "z") { e.preventDefault(); e.shiftKey ? redo() : undo(); }
    else if (mod && e.key.toLowerCase() === "s") { e.preventDefault(); doSave(); }
    else if (mod && e.key.toLowerCase() === "c") { e.preventDefault(); doCopy(); }
    else if (e.key === "Escape") close();
  }

  function askText(pt) {
    const r = cv.getBoundingClientRect();
    const inp = document.createElement("input");
    inp.type = "text";
    inp.className = "text-input";
    inp.style.left = r.left + pt.x * viewScale + "px";
    inp.style.top = r.top + pt.y * viewScale + "px";
    inp.style.fontSize = Math.max(14, size * 6 * viewScale) + "px";
    inp.style.color = color;
    document.body.appendChild(inp);
    inp.focus();
    const commit = (ok) => {
      const v = inp.value.trim();
      inp.remove();
      if (ok && v) { ops.push({ type: "text", color, width: size, points: [pt], text: v }); redoStack = []; render(); }
    };
    inp.addEventListener("blur", () => commit(true));
    inp.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter") commit(true);
      if (ev.key === "Escape") { ev.preventDefault(); commit(false); }
    });
  }

  /* ================= 命令 ================= */
  function undo() { if (ops.length) { redoStack.push(ops.pop()); render(); } }
  function redo() { if (redoStack.length) { ops.push(redoStack.pop()); render(); } }
  function close() { invoke("close_editor").catch(() => {}); }

  async function exportPng() {
    render();
    const url = cv.toDataURL("image/png");
    return invoke("save_png_from_data_url", { dataUrl: url });
  }

  async function doCopy() { try { const p = await exportPng(); await invoke("copy_image_file", { path: p.path }); toast("已复制到剪贴板"); } catch (e) { toast(String(e)); } }
  async function doSave() { try { const p = await exportPng(); const r = await invoke("save_image_as", { path: p.path }); if (r) toast("已保存：" + r); } catch (e) { toast(String(e)); } }
  async function doPin() { try { const p = await exportPng(); await invoke("open_pin", { path: p.path }); } catch (e) { toast(String(e)); } }

  root.querySelector("#undo").onclick = undo;
  root.querySelector("#redo").onclick = redo;
  root.querySelector("#clear").onclick = () => { ops = []; redoStack = []; render(); };
  root.querySelector("#fit").onclick = () => { zoom = 1; layout(); };
  root.querySelector("#copyBtn").onclick = doCopy;
  root.querySelector("#saveBtn").onclick = doSave;
  root.querySelector("#pinBtn").onclick = doPin;
  root.querySelector("#ocrBtn").onclick = () => runOcr();
  root.querySelector("#trBtn").onclick = () => openTranslate("");
  root.querySelector("#sideClose").onclick = () => { root.querySelector("#side").classList.remove("show"); layout(); };

  /* ================= OCR ================= */
  async function runOcr() {
    const side = root.querySelector("#side");
    const content = root.querySelector("#sideContent");
    const foot = root.querySelector("#sideFoot");
    side.classList.add("show"); layout();
    root.querySelector("#sideTitle").textContent = "识别结果";
    content.innerHTML = `<div class="spin">正在识别…</div>`;
    foot.innerHTML = "";
    try {
      const p = await exportPng();
      const res = await invoke("ocr", { path: p.path });
      ocrLines = res.lines || [];
      if (!ocrLines.length) { content.innerHTML = `<div class="empty">没有识别到文字</div>`; drawOcrOverlay(); return; }
      content.innerHTML = ocrLines.map((l, i) =>
        `<div class="ocr-line" data-i="${i}"><span class="box">#${i + 1}</span>${escapeHtml(l.text)}</div>`).join("");
      content.querySelectorAll(".ocr-line").forEach((el) => {
        el.onclick = async () => {
          const t = ocrLines[+el.dataset.i].text;
          await invoke("copy_text", { text: t });
          toast("已复制该行");
        };
      });
      foot.innerHTML = `<button id="copyAll">复制全部</button><button id="trAll" class="primary">翻译全部</button>`;
      foot.querySelector("#copyAll").onclick = async () => {
        await invoke("copy_text", { text: ocrLines.map((l) => l.text).join("\n") });
        toast("已复制全部文本");
      };
      foot.querySelector("#trAll").onclick = async () => {
        await invoke("copy_text", { text: ocrLines.map((l) => l.text).join("\n") });
        openTranslate(ocrLines.map((l) => l.text).join("\n"));
      };
      drawOcrOverlay();
    } catch (e) {
      content.innerHTML = `<div class="empty" style="color:#a32d2d">识别失败：${escapeHtml(String(e))}</div>`;
    }
  }

  function drawOcrOverlay() {
    octx.setTransform(dpr, 0, 0, dpr, 0, 0);
    octx.clearRect(0, 0, ovl.width, ovl.height);
    if (!ocrLines.length || !root.querySelector("#side").classList.contains("show")) return;
    octx.save();
    octx.scale(viewScale, viewScale);
    octx.strokeStyle = "rgba(216,90,48,.85)";
    octx.fillStyle = "rgba(216,90,48,.10)";
    octx.lineWidth = Math.max(1, 1 / viewScale);
    for (const l of ocrLines) {
      octx.fillRect(l.x, l.y, l.width, l.height);
      octx.strokeRect(l.x, l.y, l.width, l.height);
    }
    octx.restore();
  }

  /* ================= 翻译 ================= */
  async function openTranslate(text) {
    const side = root.querySelector("#side");
    const content = root.querySelector("#sideContent");
    const foot = root.querySelector("#sideFoot");
    side.classList.add("show"); layout();
    root.querySelector("#sideTitle").textContent = "翻译";
    content.innerHTML = `
      <div class="field">
        <label>原文</label>
        <textarea id="srcText" rows="6" placeholder="输入或粘贴要翻译的文本">${escapeHtml(text)}</textarea>
      </div>
      <div class="field">
        <label>目标语言</label>
        <select id="tgt">
          <option value="zh-CN">简体中文</option>
          <option value="en">English</option>
          <option value="ja">日本語</option>
          <option value="ko">한국어</option>
          <option value="fr">Français</option>
          <option value="de">Deutsch</option>
          <option value="es">Español</option>
        </select>
      </div>
      <div class="field">
        <label>译文</label>
        <textarea id="dstText" rows="6" placeholder="点击「翻译」" readonly></textarea>
      </div>`;
    foot.innerHTML = `<button id="swap">中英互换</button><button id="doTr" class="primary">翻译</button><button id="cpTr">复制译文</button>`;

    const srcEl = content.querySelector("#srcText");
    const dstEl = content.querySelector("#dstText");
    const tgtEl = content.querySelector("#tgt");
    srcEl.focus();

    foot.querySelector("#doTr").onclick = async () => {
      const t = srcEl.value.trim();
      if (!t) return;
      dstEl.value = "翻译中…";
      try {
        const r = await invoke("translate", { text: t, target: tgtEl.value });
        dstEl.value = r;
      } catch (e) { dstEl.value = "翻译失败：" + e; }
    };
    foot.querySelector("#cpTr").onclick = async () => { await invoke("copy_text", { text: dstEl.value }); toast("已复制译文"); };
    foot.querySelector("#swap").onclick = () => {
      const cur = tgtEl.value;
      tgtEl.value = cur === "en" ? "zh-CN" : "en";
    };
  }

  function toast(msg) {
    const t = document.createElement("div");
    t.className = "toast";
    t.textContent = msg;
    document.body.appendChild(t);
    setTimeout(() => t.remove(), 2200);
  }
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}
