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
- `js/src/websock.ts`：TS 6 下 protobuf/sodium 的 `Uint8Array` 默认 `ArrayBufferLike`，DOM `WebSocket.send` 要 `BufferSource`，发送处做类型断言（不改线上字节）。
- `js/src/globals.js`：新增 `app_name`（读 `config.js` 的 `appName`，默认 `RustDesk`）。
- `web/yuv.js`：修复软件解码 worker——原版本把 `{display, frame}` 误当 frame 解码且回包丢失 display，导致无 WebGL 环境下黑屏。
- `lib/web/bridge.dart`：补齐 web 首页/连接/会话路径上缺失的桥方法（`mainGetAppNameSync`、`mainIsOptionFixed` 等，上游重构时留下的 `UnimplementedError` 会让首页直接灰屏）；修正 `SetByName` 大小写笔误。
- `js/package.json`：固定 `libsodium`/`libsodium-wrappers@0.7.13`、`typescript@6.0.3`、`vite@7.3.6`、`@types/node@26.3.0`（npm `overrides` 钉死；0.7.16+ 的 ESM 会拆出本包没有的 `./libsodium.mjs`；不要上 Vite 8，Rolldown 不再支持这里用的 function-form `manualChunks`）。`tsconfig.json` 使用 `moduleResolution: "bundler"`、`skipLibCheck`、`stableTypeOrdering`、`types: ["node"]`（给 TS 7 铺路；本栈不装 7.0，因其尚无 Compiler API）。`vite.config.js` 把 sodium 别名到 CJS 构建，并用 `manualChunks` 打出 Flutter `index.html` 写死的 `js/dist/index.js` + `js/dist/vendor.js`（Vite 2.9 起不再默认拆 vendor，`splitVendorChunkPlugin` 在 Vite 7 已删除）。`pin-js-deps.sh` 在安装前再写一遍这些钉死版本。
- `index.html`：加载 `config.js`。

解码器包（原 `web_deps.tar.gz` 已 404）由 `fetch-codecs.sh` 重建：ogv.js 1.8.6 官方 release zip（含 SIMD，npm 包没有）、npm `yuv-canvas@1.2.6`（用 esbuild 打成浏览器 IIFE）、npm `opusscript@0.1.1` 生成 `libopus.js` 音频 worker。

本地构建依赖：node + npm、python3、yarn、protoc、Flutter 3.19.6（`FLUTTER_ROOT` 或 PATH）。`Dockerfile.web` 为一体化容器构建，无本地依赖。

## 实施经验与踩坑

本次落地过程中的非显而易见发现（v1/v2 架构、2026 工具链位腐烂、上游运行时 bug、hbbr loopback 行为、headless 测试与调试方法）都记录在 [NOTES.md](NOTES.md)。

## 已知限制

- UI 停留在 v1.2.4 时代（v1 Web 客户端）；协议与现行服务端/被控端向后兼容。
- 文件传输、终端、语音通话、TCP 隧道等未在 web 端实现（上游 v1 同样没有）。
- 音频依赖重建的 `libopus.js`；若解码异常可先在设置里关闭声音。
