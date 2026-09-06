# CardDue

**自托管的信用卡日期管理、动态日历订阅与多渠道提醒。**

Rust / Axum / PostgreSQL 后端，Svelte 5 / SvelteKit / TypeScript / Tailwind 4 前端。静态前端由 Rust 同域提供，生产运行不需要 Node.js，也不需要 Redis。

> 当前为第一版源码候选。请根据 [验证记录](docs/verification.md) 和具体提交的 GitHub Actions 结果判断测试状态，不把“有实现”等同于“所有真实设备已经验收”。CardDue 不会自动还款，不能替代银行账单，也不应作为唯一还款保障。

## 功能范围

信用卡名称、发卡行、卡组织、可选尾四位、识别色和币种；账单日、固定同月／次月还款日、账单后若干天还款；年费及单次／每月／每年自定义事项；自动账期、单期覆盖、金额记录、已还款标记与撤销；归档与恢复。

多个私密 ICS 订阅，按卡片和事件类型筛选，三级隐私模式，稳定 UID、逐订阅 SEQUENCE、ETag/304、链接撤销；Bark、SMTP 邮件、Telegram、Web Push、Webhook；MiniJinja 自定义模板、HTML 邮件、验证、隔离预览与测试发送。

PostgreSQL 持久通知队列，租约与并发领取，失败重试、已还款取消和投递历史；Argon2id 密码、服务端会话、CSRF、凭据加密、出站地址检查、限流和审计；中文响应式界面，浅色／深色／跟随系统。

不包含银行连接、消费流水同步、自动还款、信用评分、账单 OCR 或财务建议。

## 在服务器容器中验证

只需服务器已有 Git、Docker 和 Compose，无须在本机或服务器主机安装 Rust/Node：

```sh
sh scripts/verify-docker.sh
```

验证使用独立临时 PostgreSQL，执行锁定依赖的 Rust 格式检查、clippy、测试、API 导出，以及前端类型检查、测试、构建和资源预算。退出时清理测试容器。缓存清理见 [部署与运维](docs/operations-guide-zh.md)。

## 本机试用

准备 Git、Docker Engine / Compose 和用于生成私密配置的 Node.js 22.12 或更新版本。

```sh
git clone https://github.com/jzl2163/carddue.git
cd carddue
node scripts/init-env.mjs --development
docker compose up -d --build
```

访问 `http://localhost:8080`。从本地私密 `.env` 文件读取 `SETUP_TOKEN`，在首次设置页面创建管理员账户。初始化脚本会生成数据库密码、应用加密密钥、日历签名密钥、初始化令牌和 Web Push VAPID 私钥；不会覆盖已有配置，也不会把密钥打印到日志。不要将本机 HTTP 配置暴露到公网。

## HTTPS 部署

将域名指向服务器，确保 80/443 可用，然后在一个没有现存 `.env` 的新部署目录运行：

```sh
node scripts/init-env.mjs --domain card.example.com --email admin@example.com
docker compose --profile https up -d --build
```

已有反向代理时，不启用 Caddy profile，将 HTTPS 流量代理到 `127.0.0.1:8080`。`APP_BASE_URL` 必须与浏览器地址的协议、域名和端口一致，不支持域名子路径。已有部署升级时必须保留原数据库和密钥，不能重新生成配置覆盖它们。

## 第一次使用

先添加一张卡并核对实际账期，然后创建日历订阅。在“通知中心 → 通知渠道”保存一个渠道并发送测试，查看投递记录。最后创建还款规则，例如提前 `7, 3, 1, 0` 天、09:00 发送。标记已还款仅记录本人的确认，不代表银行已到账。

浏览器推送需要服务器 VAPID 配置、支持的浏览器、安全上下文及明确的通知授权。手机锁屏可能显示模板中的内容，建议使用不含金额和尾号的模板。

## 文档

| 文档 | 内容 |
|---|---|
| [用户指南](docs/user-guide-zh.md) | 页面操作、账期、还款标记、日历和日常排错 |
| [通知与模板](docs/notifications-zh.md) | 五种渠道、SMTP、Web Push、变量和发送语义 |
| [部署与运维](docs/operations-guide-zh.md) | 环境变量、HTTPS、密钥、前后端开发、备份恢复 |
| [架构说明](docs/architecture.md) | 日期、事务、队列、ICS 一致性与安全边界 |
| [API 说明](docs/api.md) | 认证、请求结构、接口和 Rust 派生类型 |
| [安全说明](SECURITY.md) | 威胁模型、秘密数据与部署边界 |
| [验收清单](docs/acceptance-checklist.md) | 自动化和真实设备验收 |
| [交接说明](HANDOFF.md) | 实现状态、运行命令、后续 Codex 工作 |
| [开发代理约定](AGENTS.md) | 不得破坏的约束与验证规则 |
| [验证记录](docs/verification.md) | 实际执行结果与尚未执行的检查 |

## 本地开发

```sh
node scripts/init-env.mjs --development
make dev-services
make frontend-install
# 终端一
make dev-backend
# 终端二
make dev-frontend
```

打开 `http://localhost:5173`。Vite 代理 `/api`、`/cal` 和 `/health` 到 Rust 的 8080 端口。开发脚本仅在 development 模式下修改进程中的数据库主机、Origin 和资源目录，不会改写 `.env`。

修改 Rust 输入模型后执行 `make api`。前端的业务输入类型来自后端 OpenAPI 生成，不手工维护重复定义。生产镜像包含 Rust 二进制和静态资源，不包含 Node、npm 或 Rust 编译器。

数据库集成测试需要独立测试 PostgreSQL 账户和 `CREATEDB` 权限，绝不能指向生产库：

```sh
export DATABASE_URL='postgres://TEST_USER:TEST_PASSWORD@localhost:5432/carddue_test'
make test
```

## 必须了解的限制

ICS 由客户端轮询，不是即时推送。服务商接收成功不代表设备已显示或用户已阅读。通知采用至少一次交付语义，服务商接收后服务端崩溃的极端窗口可能导致重复消息。周末调整不包含银行节假日和宽限政策。备份必须同时包含数据库、应用加密密钥、日历签名密钥和 VAPID 密钥。

本仓库尚未替所有者选定开源许可证；公开可见不自动授予额外再分发许可。依赖各自的许可证不受影响。
