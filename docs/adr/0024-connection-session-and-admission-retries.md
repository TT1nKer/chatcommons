# ADR 0024: Connection sessions and admission retries

Status: implemented in 0.1.0-alpha.6

## Context

A device's PeerId survives process restarts. A new connection can arrive before
libp2p has closed that device's previous connection. Peer-level authentication
flags alone then suppress the certificate/inventory needed by the new process.
libp2p request-response also distributes outbound requests across connections,
so sending another certificate without retiring the old process's path is
insufficient. A late response or failure from that path must not update the new
session's completion state.

Separately, a joining process can persist its signed acceptance before the
server commits it, or lose the server's success response after remote admission.
Rejecting every nonempty database makes these ordinary failures unrecoverable
through the invitation UI.

## Decision

For overlapping direct connections, retain the latest connection, close older
paths and start a fresh mutual authentication/inventory after their cleanup.
Ignore protocol messages from retired connections. Preserve supplemental paths
created by relay-to-direct DCUtR upgrades: those paths belong to the same running
process and retain the relay fallback. The supported alpha model remains one
running process per persisted device, not copied identities running concurrently.

Track outstanding synchronization requests. Idle completion requires a live
mutually authenticated session, no outstanding sync requests and no missing
ancestry. Transport closure invalidates completion. A closed response channel
is an ordinary per-request network failure; it must not terminate the Home
Server. Idle completion does not promise delivery to every community member.
Bootstrap and one-shot synchronization attempts each have a 30-second bound.
A repeated inventory requests the missing frontier of already-buffered events,
so a lost parent response can be retried without discarding valid child events.

When a matching invitation is retried, local signed/profile-accepted admission
is checked first. If the server already accepts this identity, authenticate and
synchronize normally. If it rejects the still-unadmitted identity, repeat the
capability proof and submit the exact same signed acceptance. Local invitation
ancestry without an acceptance may also resume bootstrap. Another identity
cannot redeem a consumed invitation, and a removed member cannot bypass the
current server authorization projection by retaining a local acceptance.

## Compatibility and boundaries

Core/profile canonical bytes, certificate verification, network message shapes,
protocol identifiers and SQLite schema are unchanged. No protocol transition or
data migration is required. This does not implement invitation expiry,
cancellation, identity recovery, automatic server discovery or continuous chat
polling. Offline clients learn membership/server changes only after receiving
the signed events; already-downloaded plaintext history cannot be revoked.

## 中文摘要

同一设备重启后可能在旧连接尚未关闭时建立新连接。直接连接重叠时先淘汰旧路径，
再重新认证和交换历史；relay 的 DCUtR 直连升级保留原有备用路径。同步成功需要有效
的双向认证、已完成的请求和完整父依赖。关闭的响应通道属于单次网络失败，不能使
Home Server 整体退出。重复历史清单会重试暂存事件缺失的父依赖。

邀请重试复用同一身份已保存的签名接受事件：服务器已接纳时直接认证同步，尚未接纳
时重新证明邀请能力并提交原事件。其他身份不能复用已兑换邀请，已移除成员仍受当前
权限投影限制。未更改协议或存储格式；未实现邀请过期/撤销、身份恢复或持续自动同步。
