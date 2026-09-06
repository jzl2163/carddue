# CardDue 交接

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
