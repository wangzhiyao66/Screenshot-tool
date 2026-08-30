# Shotly · 轻量级截图工具

一款基于 **Tauri 2（Rust + 原生 Web）** 的轻量级截图工具，支持 macOS 与 Windows。
具备**窗口 / 控件智能识别**（鼠标悬停自动勾勒窗口边框，单击即锁定该区域）等特性。

> 当前版本：**v0.1.7**

---

## 一、下载地址（v0.1.7）

| 平台 | 包类型 | 文件 | 架构 | 下载 |
| --- | --- | --- | --- | --- |
| macOS | 磁盘镜像（DMG） | `Shotly_0.1.7_universal.dmg` | **Intel + Apple 芯片通用** | [下载](https://github.com/wangzhiyao66/Screenshot-tool/releases/download/v0.1.7/Shotly_0.1.7_universal.dmg) |
| Windows | 安装包（NSIS） | `Shotly_0.1.7_x64-setup.exe` | x64 | [下载](https://github.com/wangzhiyao66/Screenshot-tool/releases/download/v0.1.7/Shotly_0.1.7_x64-setup.exe) |

- 发布页（含更新说明）：<https://github.com/wangzhiyao66/Screenshot-tool/releases/tag/v0.1.7>
- 构建记录（GitHub 官方 Runner）：<https://github.com/wangzhiyao66/Screenshot-tool/actions/runs/33318728222>

> macOS 包为 **universal（通用）** 单包，同时内嵌 `x86_64` 与 `arm64` 两种架构，Intel Mac 与 Apple 芯片 Mac 均可直接运行，无需区分下载。

---

## 二、安装方式

### macOS
1. 下载 `Shotly_0.1.7_universal.dmg` 并双击挂载。
2. 将 **Shotly** 拖入「应用程序」文件夹。
3. 首次打开时，macOS 会因 **ad-hoc 签名**（未配置 Apple 开发者证书）弹出「无法验证开发者」的 Gatekeeper 拦截。解决方法（任选其一）：
   - 在「访达」中**右键点击 Shotly → 打开**，在弹窗中点「打开」；
   - 或执行：`sudo xattr -rd com.apple.quarantine /Applications/Shotly.app`
   - 之后即可正常启动，不再提示。

### Windows
1. 下载 `Shotly_0.1.7_x64-setup.exe` 并双击运行。
2. 按 NSIS 安装向导完成安装（安装包已内置 WebView2 运行时依赖，一般无需手动安装）。
3. 由于安装包**未做代码签名**，Windows SmartScreen 可能提示「Windows 已保护你的电脑」。点击「更多信息 → 仍要运行」即可继续；后续不再拦截。

---

## 三、快捷键

### 全局快捷键（随时唤起，可在设置中修改）
| 功能 | 默认快捷键 |
| --- | --- |
| 截图 | `Command/Ctrl + Shift + A` |
| 固定区域截图 | `Command/Ctrl + Shift + R` |
| 固定（钉住截图悬浮窗） | `Command/Ctrl + Shift + V` |

### 截图覆盖层内快捷键
| 操作 | 方式 |
| --- | --- |
| 框选截图区域 | 拖拽鼠标 |
| 窗口 / 控件智能识别 | 鼠标**悬停**自动勾勒，单击即锁定该窗口 / 控件 |
| 强制手动框选（忽略识别） | 按住 `Alt` 再拖拽 |
| 微调选区 | `← ↑ → ↓` 方向键 |
| 完成并进入编辑 | `Enter` |
| 取消截图 | `Esc` |
| 开 / 关「窗口识别」 | `W` |

### 固定悬浮窗快捷键
| 操作 | 方式 |
| --- | --- |
| 关闭固定 | `Esc` |
| 放大 / 缩小 | `+` / `-` |
| 适应窗口 | `0` |

---

## 四、权限说明

### macOS
| 权限 | 是否必须 | 用途 |
| --- | --- | --- |
| **屏幕录制**（系统设置 → 隐私与安全性 → 屏幕录制） | **必须** | 读取屏幕像素以生成截图；未授权将无法截图 |
| **辅助功能**（系统设置 → 隐私与安全性 → 辅助功能） | 可选 | 启用**控件级**智能识别（按钮 / 输入框等更精细边界）；未授权时自动回退为「仅窗口级」识别，功能仍可用 |

> 授权后若仍不生效，请**完全退出并重启应用**（macOS 权限在进程重启后才生效）。

### Windows
- 通常**无需额外授权**即可截图。
- 如遇 Defender / SmartScreen 拦截，参见上方「安装方式 → Windows」处理。

---

## 五、核心功能：窗口 / 控件智能识别

截图覆盖层激活后，将鼠标悬停在任意窗口或控件上：
- 自动**聚光高亮**该窗口 / 控件，并绘制蓝色描边边框；
- 提示框显示其**标题**与**尺寸**；
- **单击**即可将选区直接锁定为该窗口 / 控件区域（按住 `Alt` 可强制手动框选）；
- 可在「设置」中开启 / 关闭，并选择识别粒度：
  - **仅窗口**：仅识别最上层窗口边界；
  - **窗口 + 控件**（默认）：在已授权辅助功能时进一步识别按钮、输入框等控件。

---

## 六、设置项

打开应用后，点击菜单 / 托盘中的「设置」可配置：
- 截图 / 固定区域 / 固定 的全局快捷键；
- OCR 识别语言（调用系统原生 OCR，不联网、不下载模型）；
- **窗口 / 控件智能识别**：开关与识别粒度；
- 辅助功能授权状态检查。

---

## 七、从源码构建（可选）

```bash
# 前置：Rust（stable）+ Node.js 20+、系统原生构建依赖（WebView2 / Xcode CLT）
git clone https://github.com/wangzhiyao66/Screenshot-tool.git
cd Screenshot-tool

# 安装前端依赖
npm install

# 开发模式
npm run tauri:dev

# 打生产包
# macOS 双架构通用包（需同时安装 aarch64 + x86_64 两个 Rust target）
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri:build -- --target universal-apple-darwin --bundles dmg

# Windows NSIS 安装包
npm run tauri:build -- --bundles nsis
```

> CI 已通过 GitHub 官方 Runner 在每次打 `v*` tag 时自动构建 **Windows NSIS + macOS 双架构通用 DMG**，并汇总发布到 GitHub Release，无需本地手动打包。

---

## 八、常见问题

- **macOS 打开提示「无法验证开发者」**：见上方「安装方式 → macOS」的 Gatekeeper 处理。
- **截图无反应 / 黑屏**：检查「屏幕录制」权限是否已授予 Shotly，并完全重启应用。
- **智能识别只识别到窗口、识别不到按钮**：需在「辅助功能」中授权 Shotly（控件级识别依赖此权限）。
- **Windows 安装被 SmartScreen 拦截**：属正常现象（未签名），点「仍要运行」即可。

---

## 许可

本项目采用 Tauri 2 技术栈构建。具体许可以仓库 LICENSE 文件为准。
