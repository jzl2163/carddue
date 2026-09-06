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
