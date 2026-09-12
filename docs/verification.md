# Oracle ARM64 验证记录

日期：2026-09-06。源码基线：0a872e4e08fd27ce4bade04dcad1bc7e28a40a39。所有执行均在 Oracle aarch64 / Debian 13 服务器的 Docker 容器内；本机未安装依赖。

工具链实测：rustc 1.98.1、Node 22.23.2、PostgreSQL 17.11、Docker 29.7.2、Compose 5.5.0。镜像标签仍会随上游更新；Cargo.lock 与 package-lock.json 锁定应用依赖，此记录不是镜像供应链固定性证明。

| 检查 | 结果 |
|---|---|
| cargo fmt --all --check | 通过 |
| cargo clippy --locked --workspace --all-targets -- -D warnings | 通过 |
| cargo test --locked --workspace --all-targets | 10 单元 + 6 PostgreSQL 集成测试通过，无忽略 |
| npm ci、API 契约生成 | 通过 |
| npm run check | 0 errors、0 warnings |
| npm test | 2 项通过 |
| npm run build、npm run budget | 通过；首次检查 gzip JS 98,824 字节、CSS 4,996 字节；复跑 JS 98,826 字节，均低于 200 KiB / 50 KiB 限制 |
| sh scripts/verify-docker.sh | 完整复跑退出 0，自动移除该项目数据库容器及网络 |
| docker build -t carddue:arm64-validation . | ARM64 release 镜像成功，126,115,027 字节 |
| 镜像实际启动 | UID/GID 10001、只读根文件系统、移除 capabilities，Docker healthy |
| 实际数据库迁移 | _sqlx_migrations 版本 1、2 均 success=true |
| HTTP 冒烟 | 健康、静态页、首次管理员、卡片创建/列表、ICS 内容通过 |

构建镜像 ID：sha256:fe80800691a650dce388295bacfa990a57a0002e5795fd600787d7a566a2d531。
HTTP 冒烟只绑定服务器回环地址。预选端口 18080 已被其他服务使用，改用 Docker 分配的临时回环端口；未修改现有服务。临时账户和数据库均用于本轮测试。

证据摘录：[Rust 测试](evidence/rust-tests.txt)、[lint](evidence/lint.txt)、[前端](evidence/frontend.txt)、[HTTP](evidence/smoke-http.txt)。完整原始执行日志保留在服务器工作目录的 *.log，不含初始化密钥输出。

## 尚未完成
没有运行浏览器端到端测试、五种真实设备推送、完整五渠道模拟覆盖、备份恢复或公网 HTTPS 部署。当前投递集成用例重点覆盖 Bark。
HTTP 页面响应正常不等于视觉与交互验收。编译和测试通过不等于生产级安全审计通过。版本仍为 V1 候选。

## 清理
本轮临时验证容器、测试数据库和网络在完成后清理。源码、验证日志、可复用构建缓存及 carddue:arm64-validation 镜像保留在 Oracle，方便继续开发。未安装本机工具链。

## 后续更新
核心浏览器交互及编辑器修复已实测，见 [本机浏览器验证](browser-validation.md)。本轮专用 carddue-browser 服务有意保持运行供用户体验，与此前已清理的临时验证容器不同。

## 2026-09-07 六项功能更新
本次在基线 55e2856 上加入注册入口确认密码、永久删除、发行地区与卡片独立时区、银行/卡组织搜索选择、无金额模板回归，以及免息期排行榜。用法见 [功能说明](features-20260907.md)。

Oracle Docker 实测：cargo fmt、严格 clippy 通过；13 项 Rust 单元测试、9 项 PostgreSQL 集成测试全部通过，无忽略；前端类型检查 0 errors / 0 warnings，2 项 Vitest 测试通过，静态构建和资源预算通过。新增迁移 0003 解除日历发布记录对已删除事件的外键依赖，保留匿名取消 UID。

scripts/browser-features.mjs 在服务器使用现有 Playwright 依赖，经回环 SSH 转发操作本机 Chrome。独立 carddue-staging 数据库中完成：普通账户注册、银行下拉无匹配/回车/焦点回归、卡组织选择、地区及独立时区保存、账户时区回退、两种排行榜模式、跨时区提示、卡片编辑与删除、390px 无整页横向溢出、重新登录及数据保留；未发现浏览器运行时异常。截图保留服务器，未下载或据此声称完成全面视觉检查。

已验证镜像：sha256:7a252265ea450ffedba3c86665d2e3d3a15a10e646315cb18204354af27c6bf8。
详细日志留在 Oracle 的 /opt/carddue-next/feature-rust.log、feature-frontend.log、feature-image.log、feature-browser.log。
本机没有安装任何项目依赖。真实推送目的端、完整浏览器覆盖和备份恢复演练仍待验收；版本仍为候选版。

## 2026-09-07 公网更新结果
已将上述功能镜像部署到现有 CardDue Docker 服务并启用 ALLOW_REGISTRATION。上线前在服务器私有备份目录保存了最新数据库、自有配置和旧镜像，使用 pg_restore --list 确认备份可读（不等同于完整恢复演练）。
命令行 HTTPS 验收通过：健康接口、注册状态、排行榜页面、现有测试账户登录与 Secure Cookie、鉴权排行榜 API、旧卡片的可选 region/timezone 字段兼容。测试会话已退出，没有修改线上卡片；SQL 迁移 1、2、3 均 success=true，应用和数据库均 healthy。
隔离的 carddue-staging 容器、网络和虚构测试数据库卷，以及 carddue-verify 临时数据库和前端安装容器均已清理。生产数据库、源码、日志、可复用缓存和回滚备份保留。后续回归优先使用 API 与命令行，仅在交互问题需要时运行浏览器脚本。
回滚注意：旧版 CardInput 拒绝未知字段；新卡含 region/timezone 后，不能仅切换旧镜像，应同时规划兼容数据处理或匹配的备份恢复。

## 总览排行模块验证
RankingPreview 复用既有排行榜 API，按今天消费免息期排序取前三，独立处理加载/空态/错误；共享类型保持与完整排行一致。Oracle Docker 构建完成，svelte-check 为 0 errors / 0 warnings，静态构建与资源预算通过。镜像 sha256:681d4e69a452a90bd4f20929a97c7c67cc1a56109184716145e58eb5f0ca6ab2。本轮采用源码检查、构建和部署接口检查，没有新增浏览器操作或镜像实现逻辑的单元测试。


## 2026-09-12：账期范围与重复事项

- 账期预生成范围缩为本期及未来 12 个月，仍保留过去三个月的初始窗口与已有历史。旧版本生成的远期空账期从接口列表隐藏；已填写金额、最低还款额、备注、日期覆盖或还款标记的记录仍可查看。底层记录不删除，日历事件通过原有取消身份机制退出窗口。
- 事项新增 quarterly / semiannual（从开始月份起每 3 / 6 月）、custom_months（每年指定月份）及 custom_dates（最多 100 个独立日期）。月份、日期去重及范围验证在服务端执行；日号超过月底时截到月底，不产生后续漂移。旧 monthly / yearly / one_time 数据与事件身份兼容。
- 自定义日期使用日期本身作为稳定身份；移除日期保留 inactive 日历事件，用于订阅取消及待发通知撤销。提醒继续使用卡片有效时区。
- 验证在 Oracle ARM64 Docker 与独立临时数据库中进行；无本机依赖安装，无真实消息发送，无 computer use。Rust fmt / strict clippy 通过，16 个单元测试及 10 个 PostgreSQL 集成测试通过；前端 Svelte 检查 0 错误 0 警告、2 个 Vitest 测试、静态构建与资源预算检查通过。
