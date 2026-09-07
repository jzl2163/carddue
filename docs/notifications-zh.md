# 通知和模板

渠道配置保存在服务器并加密。页面列表不会返回完整明文凭据。

| 渠道 | 必需配置及前提 |
|---|---|
| Bark | 服务 base_url 和 device_key；HTTPS 服务可达 |
| SMTP 邮件 | host、port、security、from、to；需要认证时提供 username/password；生产使用 tls 或 starttls |
| Telegram | bot_token、chat_id；目标需允许机器人发送消息 |
| Web Push | 浏览器订阅、服务器 VAPID 私钥与联系地址；安全上下文和用户授权 |
| Webhook | HTTPS url，可选 bearer_token；接收端按通知 id 去重 |

模板使用 MiniJinja，标题、正文及渠道相关字段可自定义。可用变量以模板编辑器及后端模板上下文为准。先验证与预览，再发送到专用测试目的端。HTML 内容经过限制和清理，不能把模板当作任意代码执行环境。

队列持久化于 PostgreSQL，失败按类型重试。已还款会取消相关提醒；服务商接受消息与设备显示是不同阶段。崩溃窗口可能导致重复投递，属于至少一次交付语义。
出站默认只允许公网 HTTPS；自托管私网服务需要管理员明确配置 NOTIFICATION_PRIVATE_HOSTS。ALLOW_INSECURE_NOTIFICATIONS 仅适用于受控测试环境。

本轮自动测试不能证明所有真实渠道可用。完整设备验收仍待配置专用目的端后执行。
