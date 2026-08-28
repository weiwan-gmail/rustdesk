# v2 Web 客户端高速化优化方案（B1/B2 设计稿）

本文是 v2 视频链路**计划中的性能优化**设计稿（尚未实施）。目标是消掉当前浏览器端视频解码/渲染的几个 CPU 瓶颈。阅读前先了解当前链路（见 [NOTES.md](NOTES.md) 与 PR 里的视频传输原理说明）。

> 状态：**设计/规划**，未改动代码。

## 一、当前链路的性能瓶颈（为什么要优化）

当前 v2 的视频路径（黑屏 bug 修复后）：

```
被控: 截屏 → VP9/VP8 编码 → protobuf VideoFrame → secretbox → WS → relay
浏览器: WS收 → 解密 → protobuf解 → ogv.js(wasm软解) → I420
       → i420ToRgba(软件,CPU) → onRgba → Dart decodeImageFromPixels(又一次CPU拷贝)
       → ui.Image → CustomPaint
```

四个真正的开销点（按影响排序）：

1. **ogv.js wasm 软解 VP9/VP8**：纯 CPU、无硬解，大分辨率（1080p+）下是主瓶颈。
2. **软件 I420→RGBA 转换**（`i420ToRgba`）：每帧全屏像素逐点转换，CPU 密集。
3. **`decodeImageFromPixels` 每帧再拷贝/实例化一次 `ui.Image`**：又一轮 CPU 开销。
4. **WS over TCP relay**：可靠+有序传输，丢包时队头阻塞；视频流量全压中继带宽。

B1/B2 针对 1/2/3（浏览器端解码+渲染），不动协议、不动被控端、不动传输。

## 二、B1：WebCodecs 硬解替代 ogv.js（核心收益）

### 原理

ogv.js 是 wasm 软解；浏览器的 **WebCodecs `VideoDecoder`** 能**硬件解码** H264/VP9/AV1。当前协议里 `supported_decoding` 本来就支持声明这些编码（`message.proto` 里 VP9/H264/H265/VP8/AV1 都在），被控端也能编码 H264/AV1（`libs/scrap` 的 hwcodec/aom）。所以只需：主控声明支持 → 被控端改发对应编码 → 浏览器用 WebCodecs 硬解。

### 设计

- **解码器抽象**：`flutter/web/js/src/codec.ts` 新增一个统一的 `VideoDecoderIf` 接口（`processFrame`/`frameBuffer`/`close`），两个实现：
  - `WebCodecsDecoder`（优先）：`new VideoDecoder({codec, ...})`，`decode(EncodedVideoChunk)`。
  - `OgvDecoder`（兜底）：现有 ogv.js 路径。
- **能力检测 + 回退**（兼容性底线）：
  ```ts
  const useWebCodecs = typeof VideoDecoder !== "undefined";
  ```
  没有 WebCodecs（HTTP 非安全上下文）就回退 ogv.js——**HTTP 内网部署不能因此黑屏**。
- **编码协商**：`connection.ts` 的 `supported_decoding` 在 WebCodecs 可用时声明 H264/VP9/AV1，否则只声明 VP9/VP8（ogv.js 能力）。
- **codec 字符串映射**：VP9→`vp09.00.10.08`、H264→`avc1.42E01E`（Annex B 需转 AVCC 或声明 `avc` 格式）、AV1→`av01.0.04M.08`。

### 关键联调点（要先验证）

- **H264 帧格式**：确认 RustDesk 被控端发的是 Annex B 还是 AVCC、SPS/PPS 是否随关键帧下发——WebCodecs 对 `avc`/`annexb` 两种 `description` 处理不同。这是 B1 最大的不确定性。
- **关键帧请求**：WebCodecs 需要关键帧起解；利用现有的"refresh/关键帧请求"消息（`Misc.refresh_video`）在丢帧/起解时向被控端要关键帧。
- **WebCodecs 需安全上下文**（HTTPS/localhost）——HTTP 内网部署的 workaround 见 [web/ROOT-CA.md](web/ROOT-CA.md)（内网根 CA）或 localhost/SSH 转发。

## 三、B2：渲染路径去 CPU 拷贝

### 原理

B1 用 WebCodecs 后，解出的是 GPU 侧的 `VideoFrame`，**不必再回读成 RGBA 走 CPU**。现在 `onRgba` → `decodeImageFromPixels` 每帧 CPU 拷贝（瓶颈 3），且 B1 前的 `i420ToRgba` 软件转换（瓶颈 2）也省了。

### 设计

- WebCodecs 路径下，`VideoFrame` 直接 `createImageBitmap(frame)` 得到 `ImageBitmap`，或画到 `OffscreenCanvas` 再转。
- Dart 侧：`model.dart` 的 `onRgba`/`decodeAndUpdate` 增加一条"GPU 帧直传"路径——把 `ImageBitmap`（而非 RGBA 字节）交给 `ImagePainter` 绘制，省掉 `decodeImageFromPixels`。
- 与 B1 耦合：只有 GPU 侧帧才值得这么做；ogv.js 兜底路径仍走现有 RGBA。

### 改动范围

- `flutter/web/js/src/codec.ts`（解码器抽象 + WebCodecs 实现）
- `flutter/web/js/src/globals.ts`（`draw()` 分支：WebCodecs 直传 vs ogv.js 软件转换）
- `flutter/web/js/src/connection.ts`（编码协商、关键帧请求）
- `flutter/lib/models/model.dart` + `flutter/lib/utils/image.dart`（GPU 帧渲染路径）

## 四、B3（可选，传输层）：WebTransport / QUIC

WS 跑在 TCP 上（可靠+有序→队头阻塞）。**WebTransport** 跑在 HTTP/3/QUIC，支持**不可靠、无序 datagram**，适合实时视频（丢包只丢帧不卡后续）。

- **代价**：hbbs/hbbr 要加 QUIC/WebTransport 监听（改 rustdesk-server）+ 浏览器侧传输适配器；需 HTTPS。
- **定位**：侵入性中-高（动服务端），作为 B1/B2 之后、面向弱网/丢包环境的进一步升级。

## 五、不走的路线（背景）

- **A2（WebRTC RTP 媒体轨道）**：被控端要新增一条与 protobuf 视频面并行的 RTP/SRTP 媒体发送子系统 + 浏览器 `<video>`/RTCRtpReceiver 接收，侵入性最高，是"云游戏级"的终极形态，不建议第一步上。
- **A1（WebRTC DataChannel 传输）**：只换传输（P2P 打洞省中继），被控端零改动（`libs/hbb_common/src/webrtc.rs` 已有 DataChannel 传输），但**不解决解码瓶颈**——可作为 B 之后的叠加项。
- **C（noVNC/RFB）**：放弃 RustDesk 协议另起 VNC 炉灶，不推荐。

## 六、推荐实施顺序

1. **B1（WebCodecs 硬解）**：先验证 H264 帧格式（Annex B/AVCC + 参数集），做解码器抽象 + 能力检测 + ogv.js 回退。收益最大、侵入性最低。
2. **B2（渲染去拷贝）**：B1 落地后顺势做，消掉 readPixels/软件转换/decodeImageFromPixels 三重 CPU 开销。
3. **B3 / A1**：视弱网/中继带宽需求再定。

## 七、验证要点（实施时）

- 能力检测开关：强制 `?codec=ogv` / `?codec=webcodecs` 便于 A/B 对比。
- 对比指标：解码帧率、CPU 占用、首帧延迟、丢帧率（控制台 `video decoder: X ms` 日志）。
- 回退正确性：HTTP 非安全上下文必须回退 ogv.js 且不黑屏。
- 大分辨率（1080p/4K）下的流畅度对比。
