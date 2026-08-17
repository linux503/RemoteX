# RemoteX

**Fast Remote Desktop.** No account. No setup. Just connect.

官网：[https://linux503.github.io/RemoteX/](https://linux503.github.io/RemoteX/)

下载：[GitHub Releases](https://github.com/linux503/RemoteX/releases)

- macOS：下载 DMG，把 RemoteX 拖进「应用程序」，打开即可
- Windows：运行安装包（当前用户，无需管理员），装完从开始菜单打开

同一 Wi-Fi 下，两边都打开 RemoteX，输入对方设备码和临时密码即可连接。首页会列出附近设备。

## Run locally

```bash
source "$HOME/.cargo/env"
cd apps/desktop
npm install
npm run tauri dev
```

两台本机实例可用不同数据目录：

```bash
REMOTEX_DATA_DIR=/tmp/remotex-a npm run tauri dev
REMOTEX_DATA_DIR=/tmp/remotex-b npm run tauri dev
```

## v0.2

新色系、局域网自动发现、Windows 安装包、GitHub 官网。
