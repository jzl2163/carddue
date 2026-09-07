# 架构

浏览器运行 SvelteKit 静态前端，同源请求 Rust/Axum API。生产镜像不需要 Node 运行时。PostgreSQL 保存用户、会话、卡片、账期、日历发布状态、通知配置和队列。

dates 负责日期及本地时间换算；planner 生成账期和任务；calendar 生成 ICS；worker 领取、复查和投递任务；providers 实现渠道协议。API 模型通过 utoipa 导出 OpenAPI，前端通过 openapi-typescript 生成输入类型。

日期、ICS 稳定身份及任务取消需要共同维护一致性。用户归属检查、CSRF、凭据加密、模板资源限制及出站校验均在服务端执行。更多边界见 SECURITY.md，实际测试覆盖见验收记录。
