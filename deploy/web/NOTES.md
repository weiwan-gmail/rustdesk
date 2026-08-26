# Web 客户端实施经验与踩坑记录

本文记录本次私有部署 Web 客户端落地过程中**非显而易见**的技术发现与调试经验，供后续维护者参考。阅读前建议先看 [README.md](README.md) 的「构建原理」一节。

## 一、架构认知：为什么当前代码树构建不了 Web 客户端

- 上游分两代 Web 客户端：
  - **v1**：Flutter Web UI + TypeScript 协议栈（`flutter/web/js/`），曾长期跑在 rustdesk.com/web。
  - **v2**：当前 master 的 `flutter/lib` 里只有 Dart 侧 shim（`flutter/lib/web/bridge.dart`、`web_model.dart`），通过 `window.getByName/setByName` 调用一个 **JS 协议核心**——但这个 JS 核心**从未开源**（bridge.dart 里 142 个 `UnimplementedError`）。官方 rustdesk.com/web 现在跑的就是闭源 v2。
- `flutter/web/` 于 2025-07（提交 `5faf0ad3c`）被删除并加入 `.gitignore`。
- **结论**：不要试图在当前 master 上「补齐 v2」——那是重写整个 JS 协议栈。务实做法是从 git 历史取 v1 最后一个同步点构建。

### 关键提交定位

- v1 的 Flutter 源码与 JS 核心**最后一个同步提交**是 `96f41c1c02dd…`（2024-05-18，v1.2.4，Flutter 3.19.6，当时 CI 的 `build-rustdesk-web` 仍在跑）。
- 之后的「split web js to v1 and v2」（2024-06-22）把 JS 拆成 v1（兼容旧 Flutter）和 v2（"Under dev"，从没完成）。
- 判断同步点的方法：`git log --follow -- flutter/web/js/src/connection.ts`，找最后一个同时改 `flutter/lib` 和 `flutter/web/js` 的提交。

## 二、构建踩坑（2026 年工具链 vs 2024 年代码）

这些是「老代码 + 新工具链」的典型位腐烂（bitrot），`fetch-codecs.sh` 和补丁里的 `package.json` 固定就是为了解决它们：

1. **`@types/node` / TypeScript**：`ts-proto > protobufjs` 间接依赖 `@types/node`。2024 年的 `typescript@4.4`/`4.9` 无法解析 2026 年 `@types/node@26` 的 `ffi.d.ts`（需要 TS 5.2+）→ `TS1005`；当时只能钉死 `16.18.68`。现在协议栈钉 **`typescript@6.0.3`**（最后一代 JS 编译器，给 TS 7 铺路；7.0 还没有 Compiler API），因此可以改钉当前的 `@types/node@26.3.0`（npm `ts6.0` dist-tag）。`skipLibCheck` 挡不住语法错误，所以仍用直接依赖 + npm `overrides`。`moduleResolution` 从已弃用的 `Node`（node10）改为 `bundler`，不要用 `"ignoreDeprecations": "6.0"`。`tsconfig` 显式 `"types": ["node"]`（TS 6 默认 `types: []`；ts-proto 生成代码会读 `globalThis.Buffer`）。TS 6 的 DOM `WebSocket.send` 要 `BufferSource`，protobuf/sodium 的 `Uint8Array` 默认 `ArrayBufferLike`，所以 `websock.ts` 发送处做类型断言（不改线上字节）。`pin-js-deps.sh` 在安装前强制写入这些钉死版本。
2. **`libsodium`/`libsodium-wrappers`**：`^0.7.9` 解析到 0.7.16，其 ESM 布局里 `libsodium-wrappers.mjs` 相对导入 `./libsodium.mjs`，但该文件在另一个包里，vite 2.8 解析失败。**解法**：精确固定 `0.7.13`（2024-05 时代的版本）。注意 yarn1 的 `resolutions` 用 `**/libsodium` 没生效，直接改 `dependencies` 里的版本号最可靠。
3. **`yarn.lock` 与 `package.json` 不同步**：该提交的 lockfile 本来就是旧的，`--frozen-lockfile` 会失败。**不要用** `--frozen-lockfile`，让 yarn 重新解析。
4. **`python` vs `python3`**：`gen_js_from_hbb.py`/`ts_proto.py` 用 `python` 调用。新系统只有 `python3`，需软链或 `python-is-python3`。
5. **`web_deps.tar.gz` 已 404**：官方解码器包没了。重建来源：
   - **ogv.js**：npm 包**没有 SIMD 构建**（`codec.js` 注释明说 "yarn add has no simd"），必须用 GitHub release 的 `ogvjs-1.8.6.zip`（仓库已转移到 `bvibber/ogv.js`）。现代浏览器 WASM SIMD 可用时会加载 `OGVDecoderVideoVP9SIMDW`，缺了它视频直接黑屏。
   - **yuv-canvas**：npm 包入口是 CommonJS（`require('./FrameSink.js')`），浏览器直接 404/报错。**解法**：`esbuild --bundle --format=iife --global-name=YUVCanvas` 打成浏览器 IIFE。
   - **`libopus.js`**：原是定制 emscripten 构建。用 npm `opusscript` 内联 + 一个约 30 行的 worker 包装（协议：收 `{channels, sampleRate}` 初始化，收 opus 包，回 PCM Float32）。

## 三、运行时缺陷（上游该提交本身就坏的）

`96f41c1c` 是「custom client 重构」中途的提交，web 端有几处真 bug，补丁已修：

1. **首页灰屏**：`WebHomePage.build` 同步调用 `bind.mainGetAppNameSync()`，而 web bridge 里它是 `throw UnimplementedError()` → release 模式白屏/灰屏无任何提示。同类还有 `mainIsOptionFixed`（PeerTabPage）、`mainLoadLanPeers` 等。**重构前的旧代码用的是硬编码 `Text("RustDesk (Beta)")`，所以官方旧构建是好的**——这也是定位思路：对比重构前后文件差异。
2. **`SetByName` 大小写笔误**：`sessionPeerOption` 调了 `js.context.callMethod('SetByName', ...)`（大写 S），JS 只有 `setByName` → NoSuchMethod。
3. **软件解码 worker 双重 bug**（无 WebGL 时黑屏的根因）：
   - `globals.js` 的 `draw()` 给 worker 发 `{display, frame}`，但 `yuv.js` 的 `I420ToARGB(currentFrame)` 把整体当 frame 用（`yb.y.bytes` 为 undefined）→ worker 内抛异常，帧被吞。
   - 即便解码成功，worker 回包 `postMessage(rgba)` 丢了 display，而主线程期望 `onRgba(e.data.display, e.data.frame)`。
   - **修法**：worker 回 `{display: currentFrame.display, frame: I420ToARGB(currentFrame.frame)}`。
   - 注意：有硬件 WebGL 的真实桌面 Chrome 走 `yuvCanvas` 路径（`preserveDrawingBuffer: true`，`readPixels` 正常），不会触发此 bug。所以**这个 bug 只在软渲染/无 GPU 环境（如 VM、部分远程环境）暴露**，官方一直没发现。

## 四、协议与服务器行为（测试拓扑的坑）

1. **`check_ws()` 语义**（`libs/hbb_common/src/websocket.rs`）：域名 → `ws(s)://domain/ws/id|/ws/relay`（路径式，走 80/443 反代）；IP → `ws://IP:21118|21119`（端口偏移 +2/+3）。补丁里 `connection.ts` 的 `getrUriFromRs` 完全对齐这套规则，并加了「空服务器 → 同源 `location.host/ws/*`」（配合内置代理实现零配置）。
2. **WS 端口**：hbbs=21118、hbbr=21119，开源版就支持（UI 里 `server-oss-not-support-tip` 已过时）。
3. **hbbr 把 loopback TCP 当 CLI 命令通道**：`relay_server.rs` 的 `handle_connection` 里 `if !ws && ip.is_loopback()` 直接按命令连接处理并关闭——**全 localhost 测试时，被控端 TCP 连 21117 会被秒关**（报 "Failed to receive public key"），而 WS（21119）不受影响。**解法**：让被控端用非回环地址（如 VM 内网 IP）注册，relay 连接源地址就不是 loopback 了。这是纯测试环境问题，生产部署被控端都是外部 IP，不会踩到。
4. **relay 配对流程**：主控 PunchHoleRequest → hbbs 转发 PunchHole 给被控 → 被控 `create_relay`（生成 uuid，经 hbbs 回 RelayResponse 给主控，同时自己连 hbbr）→ 主控连 hbbr WS → hbbr 按 uuid 配对（"got paired"）。
5. **`X-Real-IP` 信任**：hbbs/hbbr 的 WS 处理会读 `X-Real-IP`/`X-Forwarded-For` 当客户端真实 IP 且不校验——所以 21118/21119 绝不能直接暴露，必须只让反代可达。

## 五、测试环境经验（headless 被控端）

1. **改配置前先杀进程**：运行中的 RustDesk 进程退出时会把内存里的配置写回 `RustDesk2.toml`，把你手改的 `custom-rendezvous-server` 覆盖掉。正确顺序：`kill` → 改 TOML → 启动。
2. **`--option`/`--password` 需要服务权限**：先 `sudo rustdesk --service` 起系统服务；`--password` 走用户主进程 IPC。也可以直接改 `RustDesk.toml` 的 `password` 字段（明文存储，见 `config.rs` 的 `set_permanent_password`）。
3. **Xvfb 下客户端 GTK 主循环 SIGSEGV 崩溃循环**：装 `openbox` + `pulseaudio`（虚拟音频）后稳定。
4. **僵尸连接干扰视频服务**：每次测试连接不关闭会累积，导致视频服务反复 stop/start、新连接拿不到帧。每轮测试要干净重置。
5. **静态屏幕只发一帧**：RustDesk 按屏幕变化发帧，全静态的 Xvfb 只出一帧，容易被误判成黑屏。跑个 `xclock -update 1` 让屏幕持续变化。

## 六、调试方法论（无 GUI 下定位 Flutter web 问题）

本次没有靠猜，关键工具链：

1. **headless Chrome 抓控制台**：`google-chrome --headless=new --enable-logging=stderr --virtual-time-budget=... --dump-dom <url>`，console 全量落 stderr。灰屏类问题先看这里。
2. **`flutter run -d web-server`（debug 模式）**：release 的 minify 栈看不懂，debug 模式给**真实方法名**（如 `mainGetAppNameSync`），直接定位到具体未实现方法。配合 tmux 按 `r` 热重载快速迭代。
3. **CDP（Chrome DevTools Protocol）驱动 JS 桥**：`--remote-debugging-port` + `Runtime.evaluate` 直接调 `setByName('session_add_sync'/'session_start'/'login')`，绕过 Flutter UI 单独验证 JS 协议栈（rendezvous/relay/登录/视频帧）。还能 `window.curConn` 挂钩 `handleVideoFrame`/`draw`/`onRgba` 统计每级数据量，逐段定位断点。
4. **服务端日志对照**：hbbs（`update_pk` 注册）、hbbr（`New relay request`/`got paired`）、被控端 `~/.local/share/logs/RustDesk/` 三处时间线对照，判断连接卡在哪一跳。
