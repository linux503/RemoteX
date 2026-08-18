const SITE = "https://linux503.github.io/RemoteX/";

const copy = {
  en: {
    navProduct: "Product",
    navFeatures: "Features",
    navDownload: "Download",
    getApp: "Get the app",
    eyebrow: "v0.2.6 · Fast Remote Desktop",
    heroTitle: "Open.<br /><em>Copy the code.</em><br /><strong>Connect.</strong>",
    heroLead: "No account, no login. A light, simple remote desktop for Windows and macOS — enter a device code and connect.",
    downloadRemoteX: "Download RemoteX",
    seeProduct: "See the product",
    note: "No account. No setup. Just connect.",
    altHome: "RemoteX home — device ID and temporary password",
    altConnecting: "Connecting overlay",
    altSession: "Remote session with latency and speed",
    altIncoming: "Incoming connection request",
    altSettings: "RemoteX settings with a sidebar",
    m1: "No Account",
    m2: "P2P First",
    m4: "Windows ↔ macOS",
    m5: "Device ID",
    m6: "Unattended",
    shotsTitle: "Clean screens. Easy to read.",
    shotHome: "Home",
    shotHomeDesc: "Device ID + temporary password. You know how to use it in 3 seconds.",
    shotHandshake: "Handshake",
    shotHandshakeDesc: "Find device, secure handshake, P2P — each step lights up.",
    shotSession: "Session",
    shotSessionDesc: "Latency, download, upload, Direct P2P — visible the moment you connect.",
    shotConfirm: "Confirm",
    shotConfirmDesc: "When someone wants to control this computer, accept first.",
    shotSettings: "Settings",
    shotSettingsDesc: "General, connection, security — a sidebar, not a crowded strip of tabs.",
    featuresTitle: "Built to connect in one breath.",
    f1t: "No Account",
    f1d: "No login page. ID and password are everything.",
    f2t: "Cross-platform",
    f2d: "Windows and macOS control each other, same soft UI.",
    f3t: "Live stats",
    f3d: "See latency and speed. P2P or Relay is obvious.",
    f4t: "3 seconds",
    f4d: "One window covers 80% of actions. Copy, connect, done.",
    dlTitle: "Pick your system and install.",
    dlMacTitle: "Download for Mac",
    dlMacDesc: "One universal installer for Apple Silicon and Intel. Drag RemoteX into Applications, then open it.",
    dlMacBtn: "Download for Mac",
    dlWinTitle: "Download for Windows",
    dlWinDesc: "Windows 10 / 11. Installs for the current user — no admin password.",
    dlWinBtn: "Download for Windows",
    dlAll: "All releases",
    dlVersion: "Version v0.2.6",
    macFixTitle: "macOS says it’s damaged?",
    macFixDesc: "Unsigned GitHub builds get quarantined by Chrome. Drag RemoteX into Applications, then paste this in Terminal:",
    copyCmd: "Copy",
    copiedCmd: "Copied",
    footerTag: "RemoteX v0.2.6 · Fast Remote Desktop",
    title: "RemoteX — Fast Remote Desktop for Windows & macOS",
    seoTitle: "RemoteX — Fast Remote Desktop | 极速远程桌面",
    description: "No account. No setup. Just connect. RemoteX is a fast P2P remote desktop for Windows and macOS — copy a device ID and connect in seconds.",
    locale: "en_US",
  },
  zh: {
    navProduct: "产品",
    navFeatures: "功能",
    navDownload: "下载",
    getApp: "获取应用",
    eyebrow: "v0.2.6 · 极速远程桌面",
    heroTitle: "打开。<br /><em>复制设备码。</em><br /><strong>连接。</strong>",
    heroLead: "无需注册，无需登录。轻量清爽的远程桌面，Windows 与 macOS 互通，输入设备码即可连接。",
    downloadRemoteX: "下载 RemoteX",
    seeProduct: "看看产品",
    note: "无需账号。无需配置。直接连接。",
    altHome: "RemoteX 首页 — 设备码和临时密码",
    altConnecting: "正在连接",
    altIncoming: "连接请求确认",
    altSession: "远程会话，显示延迟和速度",
    altSettings: "RemoteX 设置页，左侧导航",
    m1: "无需账号",
    m2: "P2P 优先",
    m4: "Windows ↔ macOS",
    m5: "设备码",
    m6: "无人值守",
    shotsTitle: "界面干净，一眼看懂。",
    shotHome: "首页",
    shotHomeDesc: "设备码 + 临时密码，打开 3 秒就知道怎么用。",
    shotHandshake: "握手",
    shotHandshakeDesc: "查找设备、加密握手、P2P，一步一步亮起来。",
    shotSession: "会话",
    shotSessionDesc: "延迟、下载、上传、直连 P2P，连上就能看见。",
    shotConfirm: "确认",
    shotConfirmDesc: "对方要控制这台电脑时，先接受，再开始。",
    shotSettings: "设置",
    shotSettingsDesc: "通用、连接、安全，左侧导航，不再挤成一排标签。",
    featuresTitle: "一口气连上，少一步都不要。",
    f1t: "无需账号",
    f1d: "没有登录页。ID 和密码就是全部。",
    f2t: "跨平台",
    f2d: "Windows 与 macOS 互相远程，同一套浅色界面。",
    f3t: "实时状态",
    f3d: "连接后显示延迟和速度，P2P 还是中继一眼能看出来。",
    f4t: "3 秒上手",
    f4d: "一个窗口解决 80% 操作。复制，连接，完成。",
    dlTitle: "选你的系统，直接安装。",
    dlMacTitle: "下载 Mac 版",
    dlMacDesc: "一个安装包同时兼容 Apple Silicon 与 Intel。拖进「应用程序」再打开即可。",
    dlMacBtn: "下载 Mac 版",
    dlWinTitle: "下载 Windows 版",
    dlWinDesc: "Windows 10 / 11。当前用户安装，不用管理员密码。",
    dlWinBtn: "下载 Windows 版",
    dlAll: "全部版本",
    dlVersion: "当前版本 v0.2.6",
    macFixTitle: "Mac 提示已损坏 / 移到废纸篓？",
    macFixDesc: "从浏览器下载的未公证应用会被隔离。先把 RemoteX 拖进「应用程序」，再把下面命令粘贴到终端回车：",
    copyCmd: "复制",
    copiedCmd: "已复制",
    footerTag: "RemoteX v0.2.6 · 极速远程桌面",
    title: "RemoteX — 极速远程桌面，Windows 与 macOS 跨平台",
    seoTitle: "RemoteX — 极速远程桌面 | Fast Remote Desktop",
    description: "无需注册，无需登录。RemoteX 是面向 Windows 与 macOS 的极速 P2P 远程桌面，复制设备码即可连接。",
    locale: "zh_CN",
  },
};

function setMeta(selector, attr, value) {
  const el = document.querySelector(selector);
  if (el && value) el.setAttribute(attr, value);
}

function detectLang() {
  const params = new URLSearchParams(location.search).get("lang");
  if (params === "zh" || params === "en") return params;
  const saved = localStorage.getItem("remotex-lang");
  if (saved === "zh" || saved === "en") return saved;
  return navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
}

function applyLang(lang) {
  const dict = copy[lang] || copy.en;
  const pageUrl = `${SITE}?lang=${lang}`;
  document.documentElement.lang = lang === "zh" ? "zh-CN" : "en";
  document.title = dict.seoTitle;
  setMeta('meta[name="description"]', "content", dict.description);
  setMeta('meta[property="og:title"]', "content", dict.seoTitle);
  setMeta('meta[property="og:description"]', "content", dict.description);
  setMeta('meta[property="og:locale"]', "content", dict.locale);
  setMeta('meta[property="og:url"]', "content", pageUrl);
  setMeta('meta[name="twitter:title"]', "content", dict.seoTitle);
  setMeta('meta[name="twitter:description"]', "content", dict.description);
  const canonical = document.getElementById("canonical");
  if (canonical) canonical.setAttribute("href", pageUrl);
  const jsonld = document.getElementById("jsonld");
  if (jsonld) {
    try {
      const data = JSON.parse(jsonld.textContent);
      data.description = dict.description;
      data.inLanguage = lang === "zh" ? "zh-CN" : "en";
      data.url = pageUrl;
      jsonld.textContent = JSON.stringify(data);
    } catch (_) {}
  }
  document.querySelectorAll("[data-i18n]").forEach((el) => {
    const key = el.getAttribute("data-i18n");
    if (dict[key]) el.textContent = dict[key];
  });
  document.querySelectorAll("[data-i18n-html]").forEach((el) => {
    const key = el.getAttribute("data-i18n-html");
    if (dict[key]) el.innerHTML = dict[key];
  });
  document.querySelectorAll("[data-i18n-alt]").forEach((el) => {
    const key = el.getAttribute("data-i18n-alt");
    if (dict[key]) el.setAttribute("alt", dict[key]);
  });
  document.querySelectorAll("[data-i18n-src]").forEach((el) => {
    const key = el.getAttribute("data-i18n-src");
    if (key) el.src = `./shots/${lang}/${key}.png?v=20260817b`;
  });
  setMeta('meta[property="og:image"]', "content", `${SITE}og-${lang}.png`);
  setMeta('meta[name="twitter:image"]', "content", `${SITE}og-${lang}.png`);
  document.querySelectorAll(".lang button").forEach((btn) => {
    btn.classList.toggle("active", btn.getAttribute("data-lang") === lang);
  });
  const copyBtn = document.querySelector(".copy-cmd");
  if (copyBtn) {
    copyBtn.textContent = dict.copyCmd;
    copyBtn.setAttribute("data-i18n-copy", dict.copyCmd);
    copyBtn.setAttribute("data-copied", dict.copiedCmd);
  }
  localStorage.setItem("remotex-lang", lang);
  const url = new URL(location.href);
  url.searchParams.set("lang", lang);
  history.replaceState({}, "", url);
}

const current = detectLang();
applyLang(current);
document.querySelectorAll(".lang button").forEach((btn) => {
  btn.addEventListener("click", () => applyLang(btn.getAttribute("data-lang")));
});
