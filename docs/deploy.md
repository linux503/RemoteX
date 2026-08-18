# RemoteX 公网信令部署流程

把下面的占位符换成你自己的服务器信息，**不要把真实 IP、账号、密码写进仓库或公开文档**。

| 占位符 | 含义 |
|--------|------|
| `<SERVER_IP>` | 服务器公网 IP 或域名 |
| `<SSH_PORT>` | SSH 端口 |
| `<SSH_USER>` | SSH 登录用户 |

客户端信令地址格式：`ws://<SERVER_IP>:7829/ws`  
健康检查：`http://<SERVER_IP>:7829/health`

登录请用 SSH 密钥，不要在文档、截图、Git 里放密码。

## 1. 服务器要求

- Ubuntu 24.04 x86_64（或同类 Linux）
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
curl -sS http://<SERVER_IP>:7829/health
# 期望：{"ok":true,"devices":0,"sessions":0}
```

## 3. 安装编译环境（首次）

```bash
ssh -p <SSH_PORT> <SSH_USER>@<SERVER_IP>
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

安装包已内置两条公网线路。默认「自动」会测速后走更快的那条；连不上时再试另一条。也可在设置里锁定线路 1 或线路 2。

## 7. 以后更新信令

```bash
ssh -p <SSH_PORT> <SSH_USER>@<SERVER_IP>
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
