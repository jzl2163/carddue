# API

API 前缀为 /api/v1，使用服务端会话 Cookie。修改请求需要正确 Origin 和会话 CSRF 令牌（X-CSRF-Token）。错误以 JSON 返回，并包含错误代码和请求标识（如可用）。

完整契约由 Rust 源码生成：
```sh
cargo run --locked --bin carddue -- openapi > frontend/openapi.json
cd frontend
npm run api:generate
```
服务器容器验证脚本已包含这两个步骤，无需在本机安装工具。不要手工维护生成的 API 类型。
路由清单见 backend/src/web.rs；认证、卡片、通知与日历的模型见对应模块。/health/live 和 /health/ready 用于运行状态检查。
