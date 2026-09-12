# CardDue 交接

## 当前功能更新（2026-09-07）
本轮完成六项用户需求，功能用法见 docs/features-20260907.md，实际验证见 docs/verification.md 最后章节。
新增 API：DELETE /api/v1/cards/{id}/permanent 和 GET /api/v1/cards/ranking；原 DELETE /cards/{id} 继续归档。
卡片 data 中 region/timezone 可空，空时区动态跟随账户；规划和投递按有效卡时区计算。删除需要账号事务锁，并保留匿名日历取消身份。
上线需执行新增 SQL 迁移并将 ALLOW_REGISTRATION=true；保留原数据库及加密/签名/VAPID 密钥。更新前应备份，不能把测试目录的 .env 复制到线上。
scripts/browser-features.mjs 可经 CDP 回归，本轮实际连接本机 Chrome，依赖仅在 Oracle。它只允许 localhost:28180 隔离测试入口，不是覆盖所有页面的 CI 套件。
子代理 GPT-5.6 Luna Max 负责简单的前端可访问性检查和功能文档，复杂逻辑和最终验证由主代理处理。

## 历史接手记录

本轮接手基线：GitHub main 提交 `0a872e4`。Oracle 工作目录：`/opt/carddue-work`，分支：`codex/arm64-validation`。

仓库已有 Rust/Svelte 代码、锁文件、两份 SQL 迁移、10 项 Rust 单元测试、6 项数据库集成测试和2项前端单元测试。此前 README 引用的文档未入库，本轮补齐基础文档及容器验证入口。

## 复跑
在具备 Docker/Compose 的服务器上运行：
```sh
sh scripts/verify-docker.sh
```
测试数据库只在 Compose 网络内可见，数据使用 tmpfs。脚本退出时移除本验证项目容器和网络。依赖缓存及工作目录中的 target、node_modules 留在服务器供复用。详见运维文档。

## 下一批验收
- 补齐 Telegram、SMTP、Web Push 和 Webhook 的模拟服务端投递、错误码及重试测试；目前集成投递测试以 Bark 为主。
- 添加浏览器端到端测试，逐页核对新增、编辑、归档、账期覆盖、还款、订阅及模板操作。package.json 中的 test:e2e 目前没有配套用例，不能视为已验收。
- 在明确配置测试目的端后，验证五种真实渠道，特别是 Web Push 授权、锁屏、订阅失效和点击行为。
- 验证备份恢复、部署升级和长时间 worker 行为。
- 更完整地审查出站地址、并发与安全边界；现有通过用例不等于覆盖全部攻击场景。

本轮实际执行结果以 docs/verification.md 为准。尚未配置生产域名，也未向真实设备或邮箱发送通知。

## 后续浏览器实测
已在 Oracle 保持 carddue-browser 测试服务运行，并从本机 Chrome 经 SSH 隧道完成核心交互。修复五处响应式对象复制导致编辑器打不开的问题。详情及清理命令见 docs/browser-validation.md。该记录补充之前的 HTTP 验收，不表示全流程浏览器自动化已完成。

## 当前上线状态
2026-09-07：六项功能已更新到现有公网 Docker 服务，注册已启用；现有账户与卡片保留。上线后的 API 检查和三份迁移全部通过，具体证据见 docs/verification.md 最后章节。隔离测试数据库已清理。本机未安装项目依赖；后续测试优先 API/命令行，必要时才运行浏览器。

## 总览免息排行预览
总览在即将到来与我的信用卡之间加入独立排行预览，复用 /cards/ranking，排序取前三并展示时区提示。总览与排行榜共享 RankingData 类型。组件支持加载、空态和单独重试。Oracle Docker 类型检查 0 错误/0 警告，静态构建与体积预算通过；本次不涉及后端或数据库迁移。
