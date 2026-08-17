# RemoteX

**Fast Remote Desktop.** No account. No setup. Just connect.

官网：[https://linux503.github.io/RemoteX/](https://linux503.github.io/RemoteX/)

下载：[GitHub Releases](https://github.com/linux503/RemoteX/releases)

- macOS（Intel + Apple Silicon 通用包）: `RemoteX_0.2.0_universal.dmg`
- Windows: `RemoteX_0.2.0_x64-setup.exe`

## Run locally

```bash
source "$HOME/.cargo/env"
cargo run -p signaling

cd apps/desktop
npm install
npm run tauri dev
```

## v0.2

新色系、连接延迟 / 速度显示、GitHub 官网与 Win/Mac 下载入口。
