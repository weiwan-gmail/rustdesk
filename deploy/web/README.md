# RustDesk Web 客户端（私有部署）

在浏览器里直接使用 RustDesk 主控端，类似官方 <https://rustdesk.com/web/>，但完全私有部署：页面由你自己提供服务，连接走你自己的 `hbbs`/`hbbr` 服务器，不接触官方服务器。

```
浏览器  ──►  Web 服务(静态页面 + WS 代理)  ──►  hbbs:21118 (/ws/id)
                                            ──►  hbbr:21119 (/ws/relay)
```

- Web 客户端是**纯主控端**：只能用来控制别人，不能被控。
- 仅走**中继**（relay）连接（浏览器无法打洞直连）。
- 被控端用普通的 RustDesk 客户端，填你的服务器地址和 Key 即可。

## 三种使用方式

| 方式 | 场景 | 命令 |
|------|------|------|
| 一、compose 一键部署 | 服务器上完整部署（含 hbbs/hbbr） | `docker compose up -d` |
| 二、集中门户挂载 | 挂到已有网关/门户的子路径 | 见 `portal/` 示例 |
| 三、本机单二进制 | 类 noVNC 的即开即用 | `./rustdesk-web --server <服务器> --open` |

### 方式一：compose 一键部署（推荐）

需要 Docker 与 Docker Compose。

```bash
cd deploy/web
cp .env.example .env      # 按需修改
docker compose up -d
```

然后浏览器打开 `http://<服务器IP>:8080`（默认 `WEB_PORT=8080`）。

`.env` 关键项：

- `SITE_ADDRESS`：站点地址。
  - `http://:80` 或 `http://192.168.1.10` → 纯 HTTP（内网免证书，默认）。
  - `rustdesk.example.com`（域名）→ Caddy 自动签发 HTTPS，此时把 compose 里 web 的端口映射改成 `80:80` 和 `443:443`。
- `RELAY_SERVER`：本服务器的公网地址（`IP:21117` 或 `域名:21117`），原生被控端用它连中继。留空时 hbbs 自动推导（中继 = hbbs 同主机的 21117），大多数单机部署留空即可。
- `RUSTDESK_SERVER`：写入 Web 客户端的默认服务器。留空 = 同源代理（推荐，页面里的 WS 请求由 web 容器转发）。

被控端配置（设置 → 网络 → ID/中继服务器）：

- `ID 服务器` = 你的服务器地址（IP 或域名）
- `Key` = 服务器 Key（compose 首次启动后见 `data/id_ed25519.pub`）

### 方式二：集中门户挂载

把 Web 客户端挂到既有门户的子路径，例如 `https://portal.example.com/rustdesk/`：

1. 用子路径重新构建 web 镜像：`BASE_HREF=/rustdesk/ docker compose build web`（或本地 `BASE_HREF=/rustdesk/ ./build-web-client.sh`）。
2. 在既有网关上按示例配置：`portal/Caddyfile`（Caddy）或 `portal/nginx.conf`（nginx）。
   - `/rustdesk/*` → web 服务（静态页面）
   - `/ws/id`、`/ws/relay` → `hbbs:21118` / `hbbr:21119`（WS 路径保持官方约定不变，可直接复用官方反代规则）

WS 路径与页面路径相互独立：页面在子路径，WS 端点仍在站点根部的 `/ws/*`，与官方文档一致。

### 方式三：本机单二进制（noVNC 式）

一个自包含的可执行文件，内嵌全部页面资源，无需 Docker：

```bash
cd deploy/web/localserver
./build.sh                 # 先构建 web 客户端（若 dist/ 不存在），再编译当前平台二进制
./build.sh --all           # 或交叉编译 linux/amd64、linux/arm64、windows、macOS 单文件
```

使用：

```bash
./rustdesk-web                                # 零配置：http://localhost:8080 ，服务器在网页设置里填
./rustdesk-web --server 192.168.1.10 --open   # 指定服务器并自动打开浏览器
./rustdesk-web --server rustdesk.example.com --listen :9000
```

参数：

- `--server host[:port]`：RustDesk 服务器。默认端口 21116，WS 端口自动推导为 +2/+3（21118/21119）；域名无端口时走 80/443 的 `/ws/id`、`/ws/relay`（对齐官方反代约定）。默认 `localhost`。
- `--listen addr`：监听地址，默认 `:8080`。
- `--base-path path`：挂载子路径，默认 `/`（需与构建期 `BASE_HREF` 一致）。
- `--tls-cert` / `--tls-key`：可选 TLS；不提供即纯 HTTP（内网/本机免证书）。
- `--ws-id` / `--ws-relay`：显式覆盖 WS 上游地址（一般用不到）。
- `--open`：启动后自动打开浏览器。

二进制同时作为 Docker 镜像入口：`docker run --rm -p 8080:8080 <镜像> --server 192.168.1.10`。

## 内网纯 HTTP 的说明

- HTTP 页面下 `ws://` 连接不受浏览器混合内容限制，画面/键鼠等核心功能正常。
- `localhost`/`127.0.0.1` 被浏览器视为安全上下文，方式三本机访问无任何限制。
- 非 localhost 的 HTTP 站点：剪贴板走 `execCommand` 兜底（首次可能需授权/降级）、PWA 不可用（无影响）、视频解码自动回退单线程（无 `SharedArrayBuffer`，性能略降）。

## 安全说明

- **21118/21119 不要直接对公网开放**：hbbs/hbbr 信任 WS 连接上的 `X-Real-IP`/`X-Forwarded-For` 头，直连者可伪造 IP 绕过 IP 限制。compose 默认把这两个端口留在容器网络内，只经 web 容器（Caddy 会自己写 `X-Real-IP`）转发。
- 方式三直连已有服务器时，同理确保该服务器的 21118/21119 只对可信网络开放。

## 构建原理（维护者向）

上游在 2025-07 删除了开源的 `flutter/web/`（v1），当前 master 只剩 v2 的 Dart 侧 shim，配套的 v2 JS 协议核心从未开源。本目录从 git 历史中最后一个 Flutter 源码与 JS 核心同步的提交（`96f41fcc02dd…`，v1.2.4 时代）构建 v1 Web 客户端，并打补丁做私有化与修复：

`patches/0001-private-web-client.patch`（相对该提交）：

- `js/src/connection.ts`：服务器地址解析对齐原生客户端 `check_ws()`（同源/域名走 `/ws/*` 路径，IP 走端口偏移）；默认服务器改由运行时 `config.js` 注入；删除启动时探测官方 `rs-*.rustdesk.com` 的 `testDelay()`。
- `js/src/globals.js`：新增 `app_name`（读 `config.js` 的 `appName`，默认 `RustDesk`）。
- `web/yuv.js`：修复软件解码 worker——原版本把 `{display, frame}` 误当 frame 解码且回包丢失 display，导致无 WebGL 环境下黑屏。
- `lib/web/bridge.dart`：补齐 web 首页/连接/会话路径上缺失的桥方法（`mainGetAppNameSync`、`mainIsOptionFixed` 等，上游重构时留下的 `UnimplementedError` 会让首页直接灰屏）；修正 `SetByName` 大小写笔误。
- `js/package.json`：固定 `libsodium`/`libsodium-wrappers@0.7.13`（新版 ESM 布局与 vite 2.8 不兼容）与 `@types/node@^16`（新版类型语法超出 TS 4.4 解析能力）。
- `index.html`：加载 `config.js`。

解码器包（原 `web_deps.tar.gz` 已 404）由 `fetch-codecs.sh` 重建：ogv.js 1.8.6 官方 release zip（含 SIMD，npm 包没有）、npm `yuv-canvas@1.2.6`（用 esbuild 打成浏览器 IIFE）、npm `opusscript@0.1.1` 生成 `libopus.js` 音频 worker。

本地构建依赖：node + npm、python3、yarn、protoc、Flutter 3.19.6（`FLUTTER_ROOT` 或 PATH）。`Dockerfile.web` 为一体化容器构建，无本地依赖。

## 实施经验与踩坑

本次落地过程中的非显而易见发现（v1/v2 架构、2026 工具链位腐烂、上游运行时 bug、hbbr loopback 行为、headless 测试与调试方法）都记录在 [NOTES.md](NOTES.md)。

## 常见问题

**Q：单机部署支持 Windows 吗？**

支持。`rustdesk-web` 单二进制是纯 Go 标准库实现（无 CGO），`./build.sh --all` 会交叉编译出 `rustdesk-web.exe`（windows/amd64），拷到 Windows 机器上双击或命令行运行即可：`rustdesk-web.exe --server <服务器IP> --open`。注意**构建**仍需在 Linux/macOS（或 Windows 的 WSL/Git Bash）上进行，产出的是原生 Windows 可执行文件。若要在 Windows 上跑整套服务端，hbbs/hbbr 也有官方 Windows 构建（rustdesk-server-windows），与 `rustdesk-web.exe` 配合同机运行即可。

**Q：单机部署可以不用中继（hbbr）吗？**

不可以——**Web 客户端天生必须走中继**，这是浏览器的能力限制决定的：浏览器只能发起 WebSocket 连接，无法监听入站 TCP、无法用 UDP，也就无法做 NAT 打洞直连（P2P）。RustDesk 的直连路径依赖这些能力，所以浏览器端只能经 hbbr 中继收发数据。这不是部署上的取舍，而是协议本质。

但请区分「中继组件」和「外部中继服务」：私有单机部署里 hbbs 和 hbbr **都跑在你自己这台机器上**（compose 里是两个容器，本机二进制则连你已有的 hbbs/hbbr），不依赖任何官方/外部中继。换句话说，流量全程不出你的机器，只是逻辑上必须经过 hbbr 这一跳。如果一定要浏览器端 P2P，唯一的理论出路是 WebRTC DataChannel，但 v1 Web 客户端未实现（RustDesk 的 WebRTC 也是另一套可选特性），不在本方案范围内。

**Q：被控端需要专门装什么吗？**

不需要。被控端就是普通的 RustDesk 桌面/移动客户端，在「设置 → 网络 → ID/中继服务器」里填你的服务器地址和 Key 即可。Web 客户端只能当主控（控制别人），不能被控。

**Q：我已经有 hbbs/hbbr 在跑了，还需要单独再跑一个 server 吗？**

不需要单独的机器，也不用改 hbbs/hbbr 的任何配置——它们保持原样即可。但浏览器需要两个 hbbs/hbbr 不提供的能力：

1. **托管 Web 页面的静态服务**：hbbs/hbbr 只提供协议端口，不出 HTML/JS 页面，所以总得有个东西把页面发给浏览器。
2. **仅 HTTPS 部署才需要的 WSS 反代**：hbbs/hbbr 的 `21118`/`21119` 是明文 `ws://`；一旦页面走 HTTPS，浏览器就要求 `wss://`，这时才需要在前面加一层 TLS 终止反代。

按场景看「额外要跑什么」：

- **内网纯 HTTP**：最少只要一个静态页面服务。浏览器可以**直连** `ws://你的IP:21118` 和 `:21119`（已实测，与走代理结果一致），WS 代理都能省掉。`rustdesk-web` 单二进制把「页面 + 代理」打包在一起，是最省事的一个文件。
- **HTTPS**：在静态服务基础上加 TLS 反代——Caddy/nginx，或 `rustdesk-web --tls-cert/--tls-key` 自带。

所以针对「已有 hbbs/hbbr」的情况，方式三的单二进制就是唯一要额外跑的东西：一个自包含 exe，`--server` 指向你现有的服务器即可，不碰 hbbs/hbbr，也不新增机器。唯一注意点：若选择让浏览器**直连** `21118`/`21119`（不经代理），这两个端口就得对浏览器网络开放，而它们信任 `X-Real-IP` 且不校验——可信内网可以接受，公网环境务必只让反代可达（见上文「安全说明」）。

## 已知限制

- UI 停留在 v1.2.4 时代（v1 Web 客户端）；协议与现行服务端/被控端向后兼容。
- 文件传输、终端、语音通话、TCP 隧道等未在 web 端实现（上游 v1 同样没有）。
- 音频依赖重建的 `libopus.js`；若解码异常可先在设置里关闭声音。
