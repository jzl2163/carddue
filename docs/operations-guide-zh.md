# 部署与运维

## Oracle 上运行验证
本轮所有工具链、下载依赖、数据库和构建产物均在服务器。主机只需 Git、Docker 和 Compose。
```sh
cd /opt/carddue-work
sh scripts/verify-docker.sh
```
独立配置 docker-compose.verify.yml 不读取生产 env_file、不发布数据库端口。数据库是临时测试库，不能保存真实资料。
脚本结束自动清理本验证项目容器及网络。需要删除此项目的 Cargo 下载缓存时：
```sh
docker compose -f docker-compose.verify.yml down -v --remove-orphans
```
源码目录中的 target、frontend/node_modules、frontend/.svelte-kit、frontend/build 属于可重建产物，确认没有构建进程使用后可删除。不要使用全局 docker system prune 清理其他应用。

## 配置与部署
用 Node 容器生成配置，无须主机安装 Node：
```sh
docker run --rm -v "$PWD:/work" -w /work node:22-bookworm-slim node scripts/init-env.mjs --development
```
生产改用 --domain 和 --email 参数，参见 README。脚本不覆盖已有 .env，权限为600。不要粘贴 .env 到公开日志。
APP_BASE_URL 必须是浏览器实际访问的 origin。生产需要 HTTPS；HTTP 只允许 development 的 loopback 地址。APP_PORT 控制宿主机回环端口。数据库默认仅容器内访问。
APP_ENCRYPTION_KEY、ICS_SIGNING_KEY、VAPID_PRIVATE_KEY 和数据库必须一起备份。SETUP_TOKEN 用于首次建立管理员。ALLOW_REGISTRATION 默认关闭。
已有代理时将其转发到本机回环的应用端口；使用 Caddy profile 前核实 80/443 没有其他服务占用。
APP_ROLE 可为 all、web、worker；拆分 worker 时必须共享数据库和密钥。FRONTEND_DIR 指向静态构建目录。

## 检查与备份
/health/live 检查进程，/health/ready 检查数据库。查看容器健康状态和应用日志，避免公开包含私人信息的日志。
scripts/backup.sh 提供数据库备份入口；恢复前应在独立实例验证数据库与原密钥配套可用。未经过恢复演练的备份不能算作恢复保障。
