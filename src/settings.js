import { invoke } from "@tauri-apps/api/core";

export async function mount(root) {
  root.innerHTML = `
    <div class="set-root">
      <h1>Shotly</h1>
      <div class="sub">轻量级跨平台截图 · 识别文字 · 翻译 · 贴图</div>

      <div class="card">
        <h2>快捷键</h2>
        <div class="field"><label>区域截图</label><div><kbd id="hkCapture">—</kbd></div></div>
        <div class="field"><label>固定区域截图</label>
          <div><kbd id="hkFixed">—</kbd> <span class="muted">（在选区工具栏点「固定区域」设置）</span></div></div>
        <div class="field"><label>剪贴板贴图</label><div><kbd id="hkPin">—</kbd></div></div>
        <div class="row" style="margin-top:12px">
          <button id="btnCapture" class="primary">立即截图</button>
          <button id="btnPerm">检查屏幕录制权限</button>
        </div>
        <div class="field" style="margin-top:10px"><span class="desc" id="permState"></span></div>
      </div>

      <div class="card">
        <h2>文字识别（OCR）</h2>
        <div class="field">
          <label>识别语言</label>
          <select id="ocrLang">
            <option value="zh-Hans,en-US">简体中文 + 英文</option>
            <option value="zh-Hans">简体中文</option>
            <option value="en-US">English</option>
            <option value="ja-JP,en-US">日本語 + English</option>
          </select>
          <div class="desc">调用系统原生 OCR（macOS Vision / Windows OCR），不下载模型、不联网。</div>
        </div>
      </div>

      <div class="card">
        <h2>翻译</h2>
        <div class="field">
          <label>翻译服务</label>
          <select id="trProvider">
            <option value="google">Google 翻译（免费端点，无需 Key）</option>
            <option value="mymemory">MyMemory（免费，无需 Key，有额度限制）</option>
            <option value="deepl">DeepL（需 API Key）</option>
            <option value="custom">自定义 OpenAI 兼容接口</option>
          </select>
        </div>
        <div class="field" id="fKey" style="display:none">
          <label>API Key</label>
          <input type="text" id="trKey" placeholder="粘贴你的 Key">
        </div>
        <div class="field" id="fEndpoint" style="display:none">
          <label>接口地址</label>
          <input type="text" id="trEndpoint" placeholder="https://api.openai.com/v1/chat/completions">
        </div>
        <div class="field" id="fModel" style="display:none">
          <label>模型</label>
          <input type="text" id="trModel" placeholder="gpt-4o-mini">
        </div>
        <div class="desc">翻译需要联网。默认本地不存储任何译文，文本直接发往你选择的服务。</div>
      </div>

      <div class="card">
        <h2>存储</h2>
        <div class="field">
          <label>默认保存目录</label>
          <input type="text" id="saveDir" placeholder="留空 = 每次询问">
          <div class="desc">留空时，点「保存」会弹出文件选择框。</div>
        </div>
      </div>

      <button id="btnSave" class="primary">保存设置</button>
      <span id="saved" class="desc" style="margin-left:10px"></span>
    </div>`;

  const s = await invoke("get_settings");

  root.querySelector("#hkCapture").textContent = s.capture_hotkey || "—";
  root.querySelector("#hkFixed").textContent = s.fixed_hotkey || "—";
  root.querySelector("#hkPin").textContent = s.pin_hotkey || "—";
  root.querySelector("#ocrLang").value = s.ocr_lang || "zh-Hans,en-US";
  root.querySelector("#trProvider").value = s.translate_provider || "google";
  root.querySelector("#trKey").value = s.translate_key || "";
  root.querySelector("#trEndpoint").value = s.translate_endpoint || "";
  root.querySelector("#trModel").value = s.translate_model || "";
  root.querySelector("#saveDir").value = s.save_dir || "";

  const sync = () => {
    const p = root.querySelector("#trProvider").value;
    root.querySelector("#fKey").style.display = p === "deepl" || p === "custom" ? "" : "none";
    root.querySelector("#fEndpoint").style.display = p === "custom" ? "" : "none";
    root.querySelector("#fModel").style.display = p === "custom" ? "" : "none";
  };
  root.querySelector("#trProvider").onchange = sync;
  sync();

  root.querySelector("#btnSave").onclick = async () => {
    s.ocr_lang = root.querySelector("#ocrLang").value;
    s.translate_provider = root.querySelector("#trProvider").value;
    s.translate_key = root.querySelector("#trKey").value.trim();
    s.translate_endpoint = root.querySelector("#trEndpoint").value.trim();
    s.translate_model = root.querySelector("#trModel").value.trim();
    s.save_dir = root.querySelector("#saveDir").value.trim();
    await invoke("save_settings_cmd", { settings: s });
    const el = root.querySelector("#saved");
    el.textContent = "已保存";
    setTimeout(() => (el.textContent = ""), 2000);
  };

  root.querySelector("#btnCapture").onclick = () => invoke("start_capture").catch((e) => alert(String(e)));
  root.querySelector("#btnPerm").onclick = async () => {
    const st = await invoke("check_permission_cmd");
    root.querySelector("#permState").textContent = st.granted
      ? "已授权，可以截图。"
      : (st.hint || "未授权，请在系统设置中开启后完全退出并重启本应用。");
  };
  root.querySelector("#btnPerm").click();
}
