# Alpha.6 validation / Alpha.6 验证记录

Date / 日期: 2026-11-10 (America/Toronto)

## Scope / 范围

Small-group signed text alpha: local identity, community/channel creation and
invitation via CLI, native invitation join/send, manual synchronization, SQLite
history and process restart. All new test identities and databases are temporary;
network tests use loopback QUIC and an isolated local relay. No real community,
private internship data, hosted service or live server was used or changed.

范围为小群签名文字聊天：本机身份、CLI 创建社区/频道/邀请、原生加入和发送、手动同步、
SQLite 历史及进程重启。使用独立临时身份、数据库、loopback QUIC 和本机 relay，
未使用真实社群、私有实习数据或线上服务器。

## Reproduction / 复现

Linux x86_64, Rust 1.95.0, Python 3.14. Build dependencies and Clippy were isolated
under `/tmp` after the initial system-disk-full failure; no shared cache was
removed or global dependency upgraded. Debug information/incremental compilation
were disabled for disk use, with two Cargo build jobs. A fresh source checkout
was verified using the same isolated Cargo dependency/target cache.

初次构建因系统磁盘已满而失败，随后将构建依赖和 Clippy 放入独立 `/tmp` 目录，
未删除共享缓存或升级全局依赖。禁用调试信息/增量构建并使用两个编译任务。
干净源码 checkout 复现使用同一份独立 Cargo 依赖及构建缓存。

```sh
cargo fmt --all -- --check
cargo build --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
python3 -m unittest discover -s apps/review-prototype -p 'test_*.py'
python3 scripts/release-smoke.py target/debug/chatcommons-node
```

59 Rust tests and 23 prototype tests pass. The release workflow repeats the gates
before building the three platform packages. Each packaged node also runs the
cross-platform identity/text/reopen smoke script. Native macOS/Windows GUI runtime
was not tested on this Linux host; their actual build/smoke result is recorded by
GitHub Actions, not inferred from Linux results.

59 项 Rust 测试和 23 项原型测试通过。发布工作流先复查质量 gate，再构建三平台包，
并运行成品 CLI 身份/文字/重开检查。本机为 Linux，未进行 macOS/Windows 原生 GUI
实机验收；其实际构建与 smoke 结果以 GitHub Actions 为准。

## Regression evidence / 回归证据

| Check / 检查 | Evidence / 证据 |
| --- | --- |
| Lost parent response / 父响应丢失 | New `sync_flow` regression fails on the previous implementation with an empty target event set; retry/duplicate/reopen now converge. / 旧实现目标事件为空，修复后重试、重复事件、重开均收敛。 |
| Overlapping device reconnect / 同设备连接重叠 | QUIC test retains the old process connection while a new node reopens its DB with the same device; fresh authentication completes. / 保留旧连接并用同一设备重开数据库，验证新握手。 |
| Admission retry / 加入重试 | Multi-process test persists acceptance before remote submission, retries the same event, then repeats after success; a different identity is still rejected. / 提交中断和成功后均重试原签名事件，其他身份仍被拒绝。 |
| Offline history/restart / 离线历史和重启 | Home Server is stopped/restarted between non-overlapping members, then SQLite history is downloaded. / 成员不同时在线，服务重启后仍可下载历史。 |
| Removal/migration / 移除和迁移 | Removed member is rejected; the old server persists an owner-signed replacement and exits fail-closed. / 移除后拒绝访问，旧服务持久化迁移声明并退出。 |
| PeerId/authorization / 身份与权限 | Runtime tests reject mismatched PeerId certificates and revoke member/infrastructure sync access after projection changes. / 拒绝 PeerId 不匹配，权限投影变化后撤销同步资格。 |
| Draft/layout/localization / 草稿、布局、双语 | Missing sidecar exercises the actual failed-send path without clearing drafts; both languages render the long-invite action and full send button inside 760×540. / 实际发送失败保留草稿，双语长邀请与发送按钮在最小窗口可达。 |

## Native window checks / 原生窗口检查

The Linux native client was exercised under Xvfb at 1180×760 and 760×540 with an
isolated local Home Server. Checks cover joining, online send, offline local save,
manual reconnect/upload, persisted history after client restart, and Chinese/English
feedback accessibility. No feedback was submitted or external inbox contacted.
Screenshots contain disposable test chat only; invitation capabilities and keys
are excluded from committed evidence.

在独立 Xvfb 中以 1180×760 和 760×540 检查原生客户端，使用临时 Home Server。
检查加入、在线发送、离线保存、手动重连上传、重启历史与中英反馈入口。
未提交反馈或联系外部收件箱；提交的截图仅包含临时测试文字，不含邀请能力或密钥。

## Boundaries / 边界

- Manual sync; local idle completion is not a receipt from every member.
  / 手动同步；本地空闲完成不代表所有成员已收到消息。
- One running process/device, one desktop identity/community. CLI administration.
  / 每设备一个运行进程，桌面一个身份/社区；管理使用 CLI。
- Invitations have no expiry/cancellation. Retry needs the original identity;
  it cannot recover a lost key or repair arbitrary corrupted state.
  / 邀请无过期/撤销；重试需要原身份，不能恢复密钥或修复任意损坏状态。
- Membership/server changes apply after their signed events arrive. Already
  downloaded plaintext history cannot be revoked. This is not end-to-end encryption.
  / 权限和服务器变更在收到签名事件后生效；已下载明文无法收回，未实现端到端加密。
- No new public-NAT measurement, trusted publisher signing, notarization, voice,
  attachments, multi-device identity or production durability promise.
  / 未新增公网 NAT 实测、可信发行者签名、公证、语音、附件、多设备或生产持久性承诺。
- Core/profile protocol identifiers, wire shapes and SQLite schema are unchanged.
  / 核心/配置协议标识、网络格式和 SQLite schema 未变。

Window evidence / 窗口证据:

- [Minimum chat after reconnect / 重连后最小聊天窗口](../assets/screenshots/alpha6-minimum-chat-en.png)
- [Chinese feedback in minimum window / 最小窗口中文反馈](../assets/screenshots/alpha6-minimum-feedback-zh.png)
- [Default window after restart / 重启后默认窗口](../assets/screenshots/alpha6-default-restart-zh.png)


## Release outcome / 发布结果

The tagged build completed Linux and macOS packaging and both Windows application
builds. Windows smoke assertions passed, but printing its final bilingual log
failed under cp1252, so the workflow correctly skipped Release creation. The
signed alpha.6 tag is preserved. The UTF-8 script fix is released as alpha.7.

Linux/macOS 打包和 Windows 两个应用构建已完成。Windows smoke 功能断言通过，
但最后的中文日志遇到 cp1252 编码错误，因此没有创建 Release。保留 alpha.6 签名标签，
UTF-8 日志修复改用 alpha.7 发布。
