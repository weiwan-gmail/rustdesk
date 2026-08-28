# RustDesk v2 Web 客户端（私有部署）

在浏览器里使用**与桌面端同一套代码**（当前 `flutter/` 树，Flutter 3.24.5）的 RustDesk 主控端，完全私有部署：页面由你自己提供服务，连接走你自己的 `hbbs`/`hbbr` 服务器，不接触官方服务器。

```
浏览器  ──►  Web 服务(静态页面 + WS 代理)  ──►  hbbs:21118 (/ws/id)
                                            ──►  hbbr:21119 (/ws/relay)
```

- Web 客户端是**纯主控端**：只能用来控制别人，不能被控。
- 仅走**中继**（relay）连接（浏览器无法打洞直连）。
- 被控端用普通的 RustDesk 客户端，填你的服务器地址和 Key 即可。
- 与 v1（`deploy/v1`，v1.2.4 时代冻结 UI）的区别见 [../README.md](../README.md) 和 [`deploy/README.md`](../../README.md)。

## 使用方式

| 方式 | 场景 | 命令 |
|------|------|------|
| 一、compose 一键部署 | 服务器上完整部署（含 hbbs/hbbr） | `docker compose up -d` |
| 二、一体化镜像 | 已有 hbbs/hbbr，只部署页面 | `docker build -f deploy/v2/web/Dockerfile.web ...` |
| 三、本地构建 | 开发/定制 | `./build-web-client.sh` |

### 方式一：compose 一键部署（推荐）

需要 Docker 与 Docker Compose。

```bash
cd deploy/v2/web
cp .env.example .env      # 按需修改
docker compose up -d
```

然后浏览器打开 `http://<服务器IP>:8080`（默认 `WEB_PORT=8080`）。

`.env` 关键项：

- `SITE_ADDRESS`：站点地址。`http://:80` 或 `http://192.168.1.10` → 纯 HTTP（内网免证书，默认）；`rustdesk.example.com`（域名）→ Caddy 自动签发 HTTPS，此时把 compose 里 web 的端口映射改成 `80:80` 和 `443:443`。
- `RELAY_SERVER`：本服务器的公网地址（`IP:21117` 或 `域名:21117`），原生被控端用它连中继。留空时 hbbs 自动推导（中继 = hbbs 同主机的 21117），大多数单机部署留空即可。
- `RUSTDESK_SERVER`：写入 Web 客户端的默认服务器。留空 = 同源代理（推荐，页面里的 WS 请求由 web 容器转发）。

被控端配置（设置 → 网络 → ID/中继服务器）：

- `ID 服务器` = 你的服务器地址（IP 或域名）
- `Key` = 服务器 Key（compose 首次启动后见 `data/id_ed25519.pub`）

### 方式二：一体化镜像（已有服务器）

```bash
# 构建上下文必须是仓库根目录
docker build -f deploy/v2/web/Dockerfile.web -t rustdesk-web-v2 .
docker run -p 8080:80 -e SITE_ADDRESS=http://:80 rustdesk-web-v2
```

镜像内包含：codec 包 → JS 协议栈构建 → `flutter build web` → Caddy 静态服务 + `/ws/*` 代理。容器启动时可用 `-e RUSTDESK_SERVER=<你的hbbs地址>` 指定默认服务器（留空则同源代理）。

### 方式三：本地构建（开发/定制）

依赖：Flutter **3.24.5**（`FLUTTER_ROOT` 或 PATH）、node + npm、python3（需 `python` 命令，`python-is-python3`）、protoc。

```bash
cd deploy/v2/web
./build-web-client.sh          # 产物在 deploy/v2/web/dist/
BASE_HREF=/rustdesk/ ./build-web-client.sh   # 子路径挂载时
```

`dist/` 是纯静态文件，交给任意 Web 服务器即可；但 `/ws/id`、`/ws/relay` 两个 WS 端点需要反向代理到 `hbbs:21118`、`hbbr:21119`（见 `Caddyfile`）。

#### 本机单二进制（localserver）

v2 有自己的 localserver（`deploy/v2/web/localserver`，纯静态服务 + WS 代理，与 v1 同源、内嵌 v2 客户端），产出**跨平台单文件** `rustdesk-web-v2`：

```bash
cd deploy/v2/web/localserver
./build.sh                 # 先构建 v2 客户端（若 dist/ 不存在），再编译当前平台二进制
./build.sh --all           # 交叉编译 linux/amd64、linux/arm64、windows/amd64、darwin/amd64、darwin/arm64

./rustdesk-web-v2                                # 零配置：http://localhost:8080 ，服务器在网页设置里填
./rustdesk-web-v2 --server 192.168.1.10 --open   # 指定服务器并自动打开浏览器
./rustdesk-web-v2 --server rustdesk.example.com --listen :9000
```

参数与 v1 相同：`--server host[:port]`（默认端口 21116，WS 端口自动推导 +2/+3；域名无端口时走 80/443 的 `/ws/id`、`/ws/relay`）、`--listen`、`--base-path`、`--tls-cert/--tls-key`、`--open`。

## 客户端使用

1. 打开页面后，在「Remote ID」输入被控端 ID，点连接箭头。
2. 输入被控端密码（被控端 RustDesk 窗口里显示的临时密码，或其设置的固定密码）。
3. 支持原生同款内联语法：`<id>@<服务器地址>?key=<服务器Key>`——指定非默认服务器/Key 时不用进设置页。
4. 设置（页面右上角 ⋮）：可填默认的 ID 服务器与 Key（写入浏览器 localStorage）。

## 内网纯 HTTP 的说明

- HTTP 页面下 `ws://` 连接不受浏览器混合内容限制，画面/键鼠等核心功能正常。
- `localhost`/`127.0.0.1` 被浏览器视为安全上下文，本机访问无任何限制。
- 非 localhost 的 HTTP 站点：剪贴板可能需授权/降级、PWA 不可用（无影响）、视频解码回退单线程（无 `SharedArrayBuffer`，性能略降）。

## 安全说明

- **21118/21119 不要直接对公网开放**：hbbs/hbbr 信任 WS 连接上的 `X-Real-IP`/`X-Forwarded-For` 头，直连者可伪造 IP 绕过 IP 限制。compose 默认把这两个端口留在容器网络内，只经 web 容器（Caddy 会自己写 `X-Real-IP`）转发。
- 单二进制直连已有服务器时，同理确保该服务器的 21118/21119 只对可信网络开放。

## 构建原理（维护者向）

v2 直接编译仓库根的 `flutter/` 树，**不 vendor 任何源码快照**：

- `flutter/web/` 是 v2 的 Web 根（重新纳入 git 跟踪）：`index.html`（Flutter 3.24 `flutter_bootstrap.js` 加载器 + `config.js` + codec 包）、`js/`（TypeScript 协议栈）。
- `js/` 的协议代码由**当前树**生成：`ts_proto.py` 对当前 `libs/hbb_common/protos` 跑 ts-proto；`gen_js_from_hbb.py` 从当前 `src/lang`、`src/client.rs`、`Cargo.toml` 生成语言表/键映射/版本号。协议跟随当前树自动演进。
- JS 栈实现 `flutter/lib/web/bridge.dart` 的 `setByName`/`getByName` 命令面，并通过 `window.onGlobalEvent`/`onRgba` 等回调推事件与视频帧给 Dart 侧。
- 依赖钉死（与 v1 相同的现代化工具链）：`typescript@6.0.3`、`vite@7.3.6`、`libsodium(-wrappers)@0.7.13`、`@types/node@26.3.0`；`vite.config.js` 用函数式 `manualChunks` 打出 `index.html` 写死的 `js/dist/index.js` + `vendor.js`。
- `flutter build web` 会把 `web/` 整个复制进产物，`build-web-client.sh` 收集时裁掉 `node_modules`/TS 源码，只留 `js/dist`。
- 解码器包由 `deploy/v2/fetch-codecs.sh` 重建（ogv.js 1.8.6 release zip 含 SIMD、yuv-canvas esbuild IIFE、opusscript 生成 libopus.js），与 v1 相同。

开发经验与踩坑记录见 [../NOTES.md](../NOTES.md)。

## 已知限制

- 已支持：outgoing 远程桌面（画面、键鼠、剪贴板、聊天、音频、显示器切换、画质选项）。
- 暂未实现：文件传输与终端页面的具体操作、地址簿同步、账号登录（OIDC）、LAN 发现、语音通话、被控（浏览器无法被控）。
