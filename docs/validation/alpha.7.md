# Alpha.7 validation / Alpha.7 验证记录

Date / 日期: 2026-11-10 (America/Toronto)

Alpha.7 contains the alpha.6 text reliability fixes and a UTF-8 logging fix in
`scripts/release-smoke.py`. The Rust implementation, wire formats and database
schema are unchanged from alpha.6; product/bundle version labels advance to 7.

Alpha.7 包含 alpha.6 的文字聊天可靠性修复，并修复 smoke 脚本的 UTF-8 日志。
Rust 功能实现、协议格式与数据库 schema 未变，产品/包版本标识更新为 7。

## Windows failure and regression / Windows 失败与回归

The [alpha.6 workflow](https://github.com/TT1nKer/chatcommons/actions/runs/42882903281)
completed both Windows release builds and all identity/text/reopen assertions.
The script then failed while printing Chinese to cp1252 stdout. Release creation
was correctly skipped. Its signed tag remains unchanged; no alpha.6 Release was
published. Local `PYTHONIOENCODING=cp1252` reproduced the same UnicodeEncodeError
before the fix and passed after explicitly configuring UTF-8 stdout.

alpha.6 的 Windows 两个程序均已构建，身份、文字和重开断言也通过；最后输出中文
成功提示时遇到 cp1252 编码错误，工作流正确阻止 Release 创建。保留原签名标签，
未发布 alpha.6 Release。本机强制 cp1252 复现同一错误，配置 UTF-8 后通过。

The regression now runs in main/PR and pre-release verify CI under cp1252. Every
platform also runs its actual release node through the smoke script. This is a
CLI runtime check, not macOS/Windows GUI acceptance.

该回归已加入 main/PR 和发布验证 gate，以 cp1252 运行；各平台也检查实际成品 CLI。
这不是 macOS/Windows GUI 实机验收。

## Fresh-source gates / 干净源码检查

A fresh source checkout passes format, locked workspace build, Clippy with warnings
as errors, 59 Rust tests, 23 prototype tests, and the cp1252 smoke. Linux x86_64,
Rust 1.95.0 and Python 3.14 were used with the isolated Cargo/Clippy environment
described in the [alpha.6 record](alpha.6.md). No shared cache was deleted or
system dependency upgraded.

干净源码 checkout 通过格式、locked 全工作区构建、Clippy 零警告、59 项 Rust 测试、
23 项原型测试和 cp1252 smoke。环境为 Linux x86_64、Rust 1.95.0、Python 3.14，
沿用 alpha.6 的独立 Cargo/Clippy 目录，未删除共享缓存或升级系统依赖。

```sh
cargo fmt --all -- --check
cargo build --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
python3 -m unittest discover -s apps/review-prototype -p 'test_*.py'
PYTHONIOENCODING=cp1252 python3 scripts/release-smoke.py target/debug/chatcommons-node
```

The alpha.7 Linux native client was checked again at 1180×760 and 760×540 with
fresh temporary identities and a loopback Home Server: bilingual invitation join,
signed send, owner download, client restart and bilingual feedback accessibility.
No feedback was submitted. The alpha.6 offline/reconnect, lost-response and live
permission evidence also applies to the unchanged Rust implementation.

alpha.7 原生 Linux 客户端以新的临时身份和 loopback Home Server，在默认和最小
窗口重新检查双语邀请加入、签名发送、所有者下载、客户端重启和双语反馈入口。
未提交反馈。alpha.6 的离线重连、响应丢失和权限测试适用于未改动的 Rust 功能实现。

## Boundaries / 边界

Test identities and small-group text only; manual sync is not an all-member delivery
receipt. One process/device and one desktop identity/community; administration uses
CLI. Invitations have no expiry/cancellation. No end-to-end encryption, identity
recovery, multi-device identity, voice or attachments. macOS/Windows GUI runtime
and new public-NAT combinations were not tested here. Binaries have no trusted
publisher signature or Apple notarization. No live server was deployed or changed.

仅测试身份和小群文字；手动同步不代表所有成员已收到消息。每设备一个进程，桌面
一个身份/社区，管理使用 CLI。邀请无过期/撤销。未实现端到端加密、身份恢复、多设备、
语音或附件；未进行 macOS/Windows GUI 和新增公网 NAT 实测。二进制没有可信发行者
签名或 Apple 公证，未部署或更改线上服务器。

Window evidence / 窗口证据:

- [Minimum chat / 最小聊天窗口](../assets/screenshots/alpha7-minimum-chat-en.png)
- [Chinese feedback / 中文反馈](../assets/screenshots/alpha7-minimum-feedback-zh.png)
- [Default window after restart / 重启后默认窗口](../assets/screenshots/alpha7-default-restart-zh.png)
