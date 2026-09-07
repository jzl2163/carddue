# 本机浏览器连接 Oracle 验证

2026-09-06，使用本机已有 Google Chrome 的独立测试 profile，通过 SSH 回环隧道访问 Oracle 上的 Docker 服务。本机只使用已有 Python 标准库/CDP 驱动浏览器，没有安装 Node、Rust、Playwright 或项目依赖。

## 部署
服务器目录 /opt/carddue-work，Compose 项目 carddue-browser。应用只监听 127.0.0.1:28080，PostgreSQL 无宿主机发布端口，独立持久卷 carddue-browser_postgres_data。
APP_BASE_URL=http://localhost:28080，development 模式，专用虚构账户与卡片；不能将该配置直接公开到公网。

本机浏览器访问 http://localhost:28080。现有隧道断开后可以重新运行：
```sh
ssh -N -L 127.0.0.1:28080:127.0.0.1:28080 wei-server
```
停止测试服务但保留数据库：
```sh
cd /opt/carddue-work
docker compose -p carddue-browser stop
```
停止本轮后台隧道：
```sh
ssh -S /tmp/carddue-browser-ssh.sock -O exit wei-server
```
.env 留在服务器且未入库；本机临时登录信息文件为 /tmp/carddue-browser-login.txt，权限 600，未包含在本记录。

## 实际结果
通过真实页面表单输入、按钮点击和导航执行：
- 初始化管理员、退出登录、再次登录，确认持久卡片数据。
- 创建虚构卡片、生成账期、标记已还款及撤销。
- 将单期还款日覆盖为 2026-09-26，保存后页面显示更新。
- 创建私密订阅，通过页面链接拉取 ICS，HTTP 200 且包含覆盖日期。
- 已有卡片设置打开/保存，已有订阅打开/保存。
- 已有默认模板打开、验证预览、保存。
- 新建虚构月度权益，重新打开该事项编辑器。
- 通知规则空态、模板页面、设置与 worker 状态显示。
- 桌面 1365×900 和移动模拟 390×844 首页截图检查；移动视口 scrollWidth=390，无整页横向溢出。
- 更新应用容器后，登录会话和数据库资料保留，应用与数据库均 healthy。

## 发现并修复
点击已有默认模板时出现 DataCloneError，编辑器未打开。Svelte 的深层响应式对象是 Proxy，直接 structuredClone 会失败。
将五处编辑副本初始化改用 $state.snapshot：CardForm、订阅、权益事项、通知模板及提醒规则。
依据：[Svelte 官方 snapshot 文档](https://svelte.dev/docs/svelte/$state#state.snapshot)。

修复后在 Oracle 重新构建 ARM64 镜像（sha256:74bc047d6958cf75d1b8f8bd19f09afd339fddaed3cce4dc3fb5e6eb32327adb），Svelte 检查 0 errors/0 warnings、静态构建和资源预算通过；现有 2 项前端单元测试通过。上述已有模板、卡片、订阅、事项编辑均在本机浏览器重新操作成功。提醒规则同类复制点已修复，但因未配置渠道，本轮未创建/编辑规则。

## 限制
这是浏览器交互实测及回归记录，不是已加入 CI 的完整 Playwright 套件。未验证真实推送、所有移动页面、真实手机 Web Push、归档恢复或完整错误场景。
新建卡会生成历史账期，首页可能显示上一期未还款；这是当前行为，尚需产品确认历史账期默认状态是否符合预期。未自动标记历史账期为已还款。
