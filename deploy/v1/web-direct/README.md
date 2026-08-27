# rustdesk-web-direct（直连版 Web 客户端）

浏览器里**按 IP 直连**被控 RustDesk 客户端，**无需运行 hbbs/hbbr 服务器**。类似 noVNC 的 websockify 模式：一个自包含二进制，既发页面又把浏览器的 WebSocket 桥接到被控端的直连 TCP 端口。

```
浏览器 ──ws/wss──► rustdesk-web-direct ──裸 TCP──► 被控:21118(直连端口)
        :8081        (页面 + WS→TCP 桥)            (开了「直接IP访问」)
```

与 `deploy/v1/web`（服务器模式）完全独立：不同 exe、不同目录，可同机不同端口并存。两者共用 `deploy/v1/src`，直连由本目录 `config.js` 的 `direct: true` 打开。

## 与服务器模式的区别

| | rustdesk-web（服务器模式） | rustdesk-web-direct（直连模式） |
|---|---|---|
| 寻址 | 按 ID | 按 IP |
| 需要 hbbs/hbbr | 需要 | **不需要** |
| 网络 | 可跨 NAT/公网（走中继） | 仅局域网/IP 可达 |
| 被控端配置 | 填服务器 + Key | 开「直接 IP 访问」 |
| 加密 | 中继加密握手 | 明文（与原生直连一致） |

## 使用

```bash
cd deploy/v1/web-direct
./build.sh                 # 构建 web 客户端并编译当前平台二进制
./build.sh --all           # 交叉编译 linux/windows/macOS 单文件

./rustdesk-web-direct                          # http://localhost:8081
./rustdesk-web-direct --listen :9000 --open    # 指定端口并自动开浏览器
```

打开页面后，在「远程 ID」输入框里**直接填被控端的 IP**（如 `192.168.1.50` 或 `192.168.1.50:21118`），输入被控端密码即可连接。客户端识别到输入是 IP 就自动走直连（对齐原生客户端 `is_ip_str` 的行为），填 ID 则仍尝试走服务器。

## 被控端设置

被控端就是普通 RustDesk 客户端，开启「直接 IP 访问」：

- GUI：设置 → 网络 → 「直接 IP 访问 / Direct IP access」打开（默认端口 21118，可用 `direct-access-port` 改）。
- 或配置文件 `RustDesk2.toml` 的 `[options]` 加 `direct-server = 'Y'`。
- 确保该端口对运行 `rustdesk-web-direct` 的机器网络可达。

## 参数

- `--listen addr`：页面监听地址，默认 `:8081`。
- `--direct-port n`：允许的目标端口，默认 `21118`（被控端直连端口）。
- `--allow-cidr a,b,c`：允许直连的目标网段（默认仅回环/私网/链路本地）。
- `--allow-any`：关闭目标 IP 限制（**危险**，见下）。
- `--tls-cert/--tls-key`：可选 TLS；不提供即纯 HTTP（内网/本机免证书）。
- `--base-path`、`--open`。

## 安全说明（防 SSRF / 开放代理 / 跨域）

`/direct?target=IP:port` 会让本程序向目标发起 TCP 连接。若不加限制，任何人都能借它探测/连接内网任意 TCP 服务。因此默认：

- **目标端口被锁定**为 `--direct-port`（默认 21118），其它端口一律 403；
- **目标 IP 默认仅允许回环/私网/链路本地**（`127/8`、`10/8`、`172.16/12`、`192.168/16`、`169.254/16`、`::1`、`fc00::/7`、`fe80::/10`），可用 `--allow-cidr` 自定义；
- 目标必须是 **IP 字面量**（不接受主机名，防 DNS 重绑定）；
- `--allow-any` 会关闭所有限制，**仅限完全可信的网络**使用。

此外针对「浏览器桥」特有的风险：

- **跨域 WebSocket 防护**：`/direct` 的 WS 握手校验 `Origin` 必须与页面同源（`Host` 一致），否则拒绝——防止恶意网页借访问者的浏览器连到本机/内网的桥（类 CSRF）。
- **资源耗尽防护**：单帧上限 8 MiB（VP9 视频帧远小于此），并发桥接数上限 32，TCP 拨号 10s 超时。
- 直连为**明文协议**（与原生 RustDesk 直连一致），只在可信网络使用；要加密请用服务器模式 + WSS。

公网暴露时务必自行评估，并优先考虑用服务器模式 + WSS 反代。

## 工作原理（维护者向）

- 被控端直连端口说 RustDesk 裸 TCP 协议，消息用变长小端长度头分帧（`libs/hbb_common/src/bytes_codec.rs`：`header = (len<<2)|(head_len-1)`）。浏览器只会 WebSocket，所以 `server/websocket.go` 手写了一个极简 RFC6455 服务端（纯标准库），`server/bridge.go` 在 WS 帧与 RustDesk 长度头帧之间转换，不触碰 protobuf 内容。
- 直连跳过 rendezvous/relay 与 secure 签名握手（被控 `secure=false`）：连上后被控先发 `Hash{salt,challenge}`，主控回 `LoginRequest`（密码哈希 `sha256(sha256(pwd+salt)+challenge)`），之后进入与服务器模式完全相同的会话协议（视频/输入/剪贴板）。
- Web 客户端与服务器模式共用 `deploy/v1/src`。本目录构建时写入 `direct: true` 的 `config.js`，`connection.ts` 的 `start()` 才会把 IP 识别为 `_startDirect`（走 `/direct`）。

## 已知限制

- 仅 IP 直连、仅局域网/IP 可达；无 ID 寻址、无 NAT 穿透。
- 明文协议（与原生直连一致），只在可信网络使用。
- 文件传输、终端、语音等未在 web 端实现（同上游 v1）。
- 其余与 `deploy/v1/web` 相同（v1.2.4 时代 UI、音频依赖重建的 libopus.js）。
