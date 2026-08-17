# RemoteX

**Fast Remote Desktop.** No account. No setup. Just connect.

官网：[https://linux503.github.io/RemoteX/](https://linux503.github.io/RemoteX/)

下载：[GitHub Releases](https://github.com/linux503/RemoteX/releases)

- macOS Apple Silicon: `RemoteX_0.2.0_aarch64.dmg`
- macOS Intel: `RemoteX_0.2.0_x64.dmg`
- Windows: `RemoteX_0.2.0_x64-setup.exe`

macOS 若提示「已损坏，移到废纸篓」，把 App 拖进「应用程序」后在终端执行：

```bash
xattr -cr /Applications/RemoteX.app && open /Applications/RemoteX.app
```

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
