# RustDesk Web 客户端部署（v1 / v2）

本目录提供两代 Web 客户端的私有部署。**v1 与 v2 互不依赖、可并存**，供对比使用。

| | **v1**（`deploy/v1`） | **v2**（`deploy/v2`） |
|---|---|---|
| UI 代码 | v1.2.4 时代冻结快照（vendor 在 `deploy/v1/src`） | 仓库**当前** `flutter/` 树（与桌面端同代码） |
| Flutter | 3.19.6 | 3.24.5 |
| 协议代码 | 冻结 proto（vendored 生成物） | 从当前 `libs/hbb_common/protos` 实时生成 |
| 界面风格 | 旧版连接页 | 与当前桌面 GUI 对齐（连接页 + 桌面版远程页/工具栏） |
| 功能 | outgoing 远控（画面/键鼠/剪贴板/聊天/音频） | 同左 + 2FA、`<id>@<server>?key=` 内联语法、跟随当前协议特性 |
| 交付方式 | 静态页 + WS 代理 / Caddy / compose / localserver 单二进制 / portal 子路径 | 静态页 + WS 代理 / Caddy / compose / 复用 v1 localserver |
| 运行时契约 | `config.js`、`/ws/id`、`/ws/relay`、`SITE_ADDRESS`、`BASE_HREF` | **完全相同** |
| 适用 | 要一个稳定、冻结、已长期验证的 Web 客户端 | 要 Web 端界面/协议跟随当前桌面端演进 |

## 快速开始

```bash
# v1（冻结版）
cd deploy/v1/web && cp .env.example .env && docker compose up -d

# v2（当前代码版）
cd deploy/v2/web && cp .env.example .env && docker compose up -d
```

两者默认都把 Web 页面暴露在 `8080` 端口，被控端配置相同（ID 服务器 + Key）。

## 文档索引

- v1：使用与部署 [deploy/v1/web/README.md](v1/web/README.md)；开发经验 [deploy/v1/web/NOTES.md](v1/web/NOTES.md)；目录说明 [deploy/v1/README.md](v1/README.md)
- v2：使用与部署 [deploy/v2/web/README.md](v2/web/README.md)；开发经验 [deploy/v2/NOTES.md](v2/NOTES.md)；目录说明 [deploy/v2/README.md](v2/README.md)

## 选择建议

- **生产求稳** → v1（冻结、经过长期验证）。
- **要新界面/新协议特性、或希望 Web 端与桌面端同步演进** → v2。
- 两者可以跑在同一台服务器的不同端口上对比评估（注意 `WEB_PORT` 不要冲突）。
