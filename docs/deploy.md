# RemoteX 公网信令部署流程

当前线上地址：

- 健康检查：`http://23.226.134.88:7829/health`
- 客户端信令：`ws://23.226.134.88:7829/ws`

SSH 端口是 **2222**（不是 22）。不要把 root 密码写进仓库，登录请用密钥。

## 1. 服务器要求

- Ubuntu 24.04 x86_64
- 能出网（拉 GitHub、编 Rust）
- 开放 TCP `7829`
- 建议 2GB+ 内存（编译时需要；运行信令本身很轻）

## 2. 放行防火墙

```bash
ufw allow 7829/tcp
# 若装了宝塔，再在面板「安全」里放行 7829
iptables -I IN_BT -p tcp --dport 7829 -j ACCEPT
```

从你电脑验证：

```bash
curl -sS http://23.226.134.88:7829/health
# 期望：{"ok":true,"devices":0,"sessions":0}
```

## 3. 安装编译环境（首次）

```bash
ssh -p 2222 root@23.226.134.88
apt-get update
apt-get install -y git build-essential pkg-config curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
source "$HOME/.cargo/env"
```

## 4. 拉取代码并编译信令

```bash
source "$HOME/.cargo/env"
if [ -d /opt/RemoteX/.git ]; then
  git -C /opt/RemoteX fetch origin
  git -C /opt/RemoteX reset --hard origin/main
else
  git clone --depth 1 https://github.com/linux503/RemoteX.git /opt/RemoteX
fi
cd /opt/RemoteX
CARGO_TERM_COLOR=never cargo build -p signaling --release -j 1
install -m 755 /opt/RemoteX/target/release/remotex-signaling /usr/local/bin/remotex-signaling
```

## 5. systemd 开机自启

写入 `/etc/systemd/system/remotex-signaling.service`：

```ini
[Unit]
Description=RemoteX signaling hub
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=root
Environment=REMOTEX_PORT=7829
Environment=RUST_LOG=signaling=info
ExecStart=/usr/local/bin/remotex-signaling
Restart=always
RestartSec=2
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
```

启动：

```bash
systemctl daemon-reload
systemctl enable --now remotex-signaling
systemctl status remotex-signaling --no-pager
curl -sS http://127.0.0.1:7829/health
```

## 6. 客户端怎么连

两台电脑都打开 RemoteX → 设置 → 连接 → 信令服务器填：

```text
ws://23.226.134.88:7829/ws
```

然后重启 App，输入对方设备码和临时密码即可，不要求同一 Wi-Fi。

新版本会把本机 `127.0.0.1` 自动切到这台公网信令。已安装的旧版需要手动改一次。

## 7. 以后更新信令

```bash
ssh -p 2222 root@23.226.134.88
source "$HOME/.cargo/env"
git -C /opt/RemoteX fetch origin
git -C /opt/RemoteX reset --hard origin/main
cd /opt/RemoteX
cargo build -p signaling --release -j 1
install -m 755 target/release/remotex-signaling /usr/local/bin/remotex-signaling
systemctl restart remotex-signaling
curl -sS http://127.0.0.1:7829/health
```

## 8. 常用排查

```bash
systemctl status remotex-signaling --no-pager
journalctl -u remotex-signaling -n 80 --no-pager
ss -lntp | grep 7829
curl -sS http://127.0.0.1:7829/health
```

`devices` 会在有客户端连上后大于 0。画面会经这台服务器中转，注意云主机带宽。
