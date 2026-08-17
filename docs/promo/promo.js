const PROMO = {
  en: {
    home: {
      badge: "Home",
      title: "Device ID ready",
      subtitle: "Copy the code. Connect in seconds.",
      tags: ["No account", "Temp password", "3 sec setup"],
    },
    connecting: {
      badge: "Handshake",
      title: "Connecting securely",
      subtitle: "Find device → Encrypt → P2P link",
      tags: ["Step by step", "Encrypted", "P2P first"],
    },
    incoming: {
      badge: "Confirm",
      title: "Accept first",
      subtitle: "You stay in control of this Mac.",
      tags: ["Your choice", "One tap", "Safe"],
    },
    settings: {
      badge: "Settings",
      title: "Clear & organized",
      subtitle: "Sidebar navigation, not crowded tabs.",
      tags: ["General", "Connection", "Security"],
    },
    session: {
      badge: "Session",
      title: "Live stats",
      subtitle: "Latency, speed, and path — visible instantly.",
      tags: ["36 ms", "Direct P2P", "Full screen"],
    },
  },
  zh: {
    home: {
      badge: "首页",
      title: "设备码就绪",
      subtitle: "复制设备码，几秒完成连接。",
      tags: ["无需账号", "临时密码", "3 秒上手"],
    },
    connecting: {
      badge: "握手",
      title: "安全连接中",
      subtitle: "查找设备 → 加密握手 → P2P 直连",
      tags: ["分步显示", "加密传输", "P2P 优先"],
    },
    incoming: {
      badge: "确认",
      title: "先确认再连接",
      subtitle: "是否允许远程，由你决定。",
      tags: ["你的选择", "一键接受", "安全可控"],
    },
    settings: {
      badge: "设置",
      title: "设置一目了然",
      subtitle: "左侧导航，不再挤成一排标签。",
      tags: ["通用", "连接", "安全"],
    },
    session: {
      badge: "会话",
      title: "实时会话状态",
      subtitle: "延迟、速度、连接路径，连上即见。",
      tags: ["36 ms", "直连 P2P", "全屏控制"],
    },
  },
};

const OG = {
  en: {
    eyebrow: "v0.2.2 · Fast Remote Desktop",
    title: "Open. Copy. Connect.",
    lead: "No account. No setup. Just connect.",
    tags: ["Windows ↔ macOS", "P2P", "36 ms"],
  },
  zh: {
    eyebrow: "v0.2.2 · 极速远程桌面",
    title: "打开。复制。连接。",
    lead: "无需账号。无需配置。直接连接。",
    tags: ["Windows ↔ macOS", "P2P", "36 ms"],
  },
};

function params() {
  return new URLSearchParams(location.search);
}

function scene() {
  return params().get("scene") || "home";
}

function lang() {
  const l = params().get("lang");
  return l === "zh" ? "zh" : "en";
}

function isWide() {
  return scene() === "session" || params().get("layout") === "wide";
}

function renderCard() {
  const l = lang();
  const s = scene();
  const data = PROMO[l][s] || PROMO.en.home;
  const wide = isWide();
  const root = document.getElementById("promo");
  root.classList.toggle("wide", wide);
  root.dataset.lang = l;
  document.documentElement.lang = l === "zh" ? "zh-CN" : "en";

  document.getElementById("badge").textContent = data.badge;
  document.getElementById("title").textContent = data.title;
  document.getElementById("subtitle").textContent = data.subtitle;
  document.getElementById("app-shot").src = `../promo/raw/${s}.png`;

  const tags = document.getElementById("tags");
  tags.innerHTML = data.tags.map((t) => `<span>${t}</span>`).join("");
}

function renderOg() {
  const l = lang();
  const data = OG[l];
  document.documentElement.lang = l === "zh" ? "zh-CN" : "en";
  document.getElementById("eyebrow").textContent = data.eyebrow;
  document.getElementById("title").textContent = data.title;
  document.getElementById("lead").textContent = data.lead;
  document.getElementById("app-shot").src = "../promo/raw/home.png";
  const tags = document.getElementById("tags");
  tags.innerHTML = data.tags.map((t) => `<span>${t}</span>`).join("");
}

if (document.body.dataset.page === "og") {
  renderOg();
} else {
  renderCard();
}
