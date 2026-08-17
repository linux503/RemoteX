# RemoteX

**Fast Remote Desktop.** No account. No setup. Just connect.

官网：[https://linux503.github.io/RemoteX/](https://linux503.github.io/RemoteX/)

下载：[GitHub Releases](https://github.com/linux503/RemoteX/releases)

- macOS（Intel + Apple Silicon 通用包）: `RemoteX_0.2.0_universal.dmg`
- Windows: 即将推出

信令服务已内嵌在桌面端（`0.0.0.0:7829`）。两台电脑在同一局域网时：A 打开 RemoteX 做主机，B 在设置 → 连接里把信令地址改成 A 显示的 `ws://局域网IP:7829/ws`。

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

独立信令（可选）：`cargo run -p signaling`

## v0.2

新色系、连接延迟 / 速度显示、GitHub 官网与 Mac 下载入口。
