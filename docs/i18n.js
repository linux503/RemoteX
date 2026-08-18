const SITE = "https://linux503.github.io/RemoteX/";

const copy = {
  en: {
    navProduct: "Product",
    navFeatures: "Features",
    navDownload: "Download",
    getApp: "Download",
    eyebrow: "v2.0.4",
    heroTitle: "A device ID is enough.",
    heroLead: "No account. Copy the ID, enter the password, connect. Windows and macOS.",
    downloadRemoteX: "Download",
    seeProduct: "See the product",
    note: "No account. No setup. Just connect.",
    altHome: "RemoteX home with device ID and temporary password",
    altConnecting: "Connecting overlay",
    altSession: "Remote session with latency and speed",
    altIncoming: "Incoming connection request",
    altSettings: "RemoteX settings with a sidebar",
    m1: "No Account",
    m2: "P2P First",
    m4: "Windows ↔ macOS",
    m5: "Device ID",
    m6: "Unattended",
    shotsTitle: "The app, as it actually looks.",
    shotHome: "Home",
    shotHomeDesc: "Device ID and temporary password. Ready in a few seconds.",
    shotHandshake: "Handshake",
    shotHandshakeDesc: "Find the device, confirm identity, then start the session.",
    shotSession: "Session",
    shotSessionDesc: "Latency and path stay visible once you connect.",
    shotConfirm: "Confirm",
    shotConfirmDesc: "Accept before anyone can control this computer.",
    shotSettings: "Settings",
    shotSettingsDesc: "Lines, LAN, and display sit in a quiet sidebar.",
    featuresTitle: "What you need. Nothing else.",
    f1t: "No account",
    f1d: "No login page. A device ID and password are enough.",
    f2t: "Windows and Mac",
    f2d: "Either side can control the other. Same app.",
    f3t: "Live path",
    f3d: "Latency and P2P or relay stay visible.",
    f4t: "macOS permissions",
    f4d: "Opens the right System Settings panel, step by step.",
    f5t: "Clipboard",
    f5d: "Copy on one machine, paste on the other.",
    f6t: "Files",
    f6d: "Send files up to 256 MB. Saved to Downloads/RemoteX.",
    dlTitle: "Install on your computer.",
    dlMacTitle: "Mac",
    dlMacDesc: "One installer for Apple Silicon and Intel.",
    dlMacBtn: "Download for Mac",
    dlWinTitle: "Windows",
    dlWinDesc: "Windows 10 and 11. Current user install, no admin prompt.",
    dlWinBtn: "Download for Windows",
    dlAll: "All releases",
    dlVersion: "Version v2.0.4",
    macFixTitle: "macOS says it’s damaged?",
    macFixDesc: "Unsigned GitHub builds get quarantined by Chrome. Drag RemoteX into Applications, then paste this in Terminal:",
    copyCmd: "Copy",
    copiedCmd: "Copied",
    footerTag: "RemoteX for Windows and macOS",
    title: "RemoteX | Fast remote desktop for Windows and macOS",
    seoTitle: "RemoteX | Fast Remote Desktop | 极速远程桌面",
    description: "No account. No setup. Just connect. RemoteX is a fast P2P remote desktop for Windows and macOS. Copy a device ID and connect in seconds.",
    locale: "en_US",
  },
  zh: {
    navProduct: "产品",
    navFeatures: "功能",
    navDownload: "下载",
    getApp: "下载",
    eyebrow: "v2.0.4",
    heroTitle: "设备码就够了。",
    heroLead: "无需账号。复制设备码，输入密码，直接连接。Windows 与 macOS。",
    downloadRemoteX: "下载",
    seeProduct: "看看产品",
    note: "无需账号。无需配置。直接连接。",
    altHome: "RemoteX 首页，显示设备码和临时密码",
    altConnecting: "正在连接",
    altIncoming: "连接请求确认",
    altSession: "远程会话，显示延迟和速度",
    altSettings: "RemoteX 设置页，左侧导航",
    m1: "无需账号",
    m2: "P2P 优先",
    m4: "Windows ↔ macOS",
    m5: "设备码",
    m6: "无人值守",
    shotsTitle: "真实界面，不是效果图。",
    shotHome: "首页",
    shotHomeDesc: "设备码和临时密码，打开就能用。",
    shotHandshake: "握手",
    shotHandshakeDesc: "找到设备、确认身份，再开始会话。",
    shotSession: "会话",
    shotSessionDesc: "连上就能看到延迟和线路。",
    shotConfirm: "确认",
    shotConfirmDesc: "对方要控制这台电脑时，先接受。",
    shotSettings: "设置",
    shotSettingsDesc: "线路、局域网、画面，都在安静的侧栏里。",
    featuresTitle: "该有的都有，其余不要。",
    f1t: "无需账号",
    f1d: "没有登录页。设备码和密码就够了。",
    f2t: "Windows 与 Mac",
    f2d: "两边都能控制对方，同一套应用。",
    f3t: "实时线路",
    f3d: "延迟、直连或中继，一眼能看出来。",
    f4t: "macOS 权限",
    f4d: "自动打开正确的系统设置，一步一步完成。",
    f5t: "剪贴板",
    f5d: "这边复制，那边粘贴。",
    f6t: "文件",
    f6d: "最多传送 256 MB，保存到 Downloads/RemoteX。",
    dlTitle: "装到你的电脑上。",
    dlMacTitle: "Mac",
    dlMacDesc: "一个安装包同时支持 Apple Silicon 与 Intel。",
    dlMacBtn: "下载 Mac 版",
    dlWinTitle: "Windows",
    dlWinDesc: "Windows 10 / 11。当前用户安装，不用管理员密码。",
    dlWinBtn: "下载 Windows 版",
    dlAll: "全部版本",
    dlVersion: "当前版本 v2.0.4",
    macFixTitle: "Mac 提示已损坏 / 移到废纸篓？",
    macFixDesc: "从浏览器下载的未公证应用会被隔离。先把 RemoteX 拖进「应用程序」，再把下面命令粘贴到终端回车：",
    copyCmd: "复制",
    copiedCmd: "已复制",
    footerTag: "RemoteX，面向 Windows 与 macOS",
    title: "RemoteX | 极速远程桌面，Windows 与 macOS 跨平台",
    seoTitle: "RemoteX | 极速远程桌面 | Fast Remote Desktop",
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
    if (key) el.src = `./shots/${lang}/${key}.png?v=20260819b`;
  });
  setMeta('meta[property="og:image"]', "content", `${SITE}og-${lang}.png`);
  setMeta('meta[name="twitter:image"]', "content", `${SITE}og-${lang}.png`);
  document.querySelectorAll(".lang-toggle button").forEach((btn) => {
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
document.querySelectorAll(".lang-toggle button").forEach((btn) => {
  btn.addEventListener("click", () => applyLang(btn.getAttribute("data-lang")));
});
