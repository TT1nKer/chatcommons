# ChatCommons product brief

## Mission

让社区能够拥有自己的数字家园，而不被任何单一平台锁住。

> Familiar community chat, owned by the community.

## 一句话介绍

ChatCommons 是一个开源社区聊天应用：使用体验像熟悉的现代群聊，但每个
社区可以选择、运营和更换自己的服务器，不需要因为离开某个平台而重建
身份、成员关系和整个社区。

## 可以直接发给朋友的 30 秒介绍

ChatCommons 不是单纯复制 Discord 的界面，也不是强行追求完全 P2P。
它想解决的是平台锁定：Discord 上的社区依赖 Discord 永久存在，而在
ChatCommons 中，每个社区选择自己的长期在线 Home Server。这个服务器
可以由朋友、社区、企业或托管商运行；如果服务不好，社区管理员可以签名
迁移到新服务器，Community ID 和成员关系不变。成员身份和消息事件由客户端
验证，服务器短暂离线时，在线成员还可以临时直接同步。

目前协议内核、签名事件、本地 SQLite 历史、单人邀请、QUIC 同步、可替换
Home Server 和备份恢复已经实现。第一版原生桌面测试客户端和永久测试社区
也已经建立，正在进入朋友 alpha 测试；账号恢复、多设备和正式发布安全仍未完成。

## English version to share

ChatCommons is an open-source community chat app designed to feel familiar
without locking every community into one platform. Each community chooses a
long-running Home Server operated by a friend, the community, an organization,
or a hosting provider. If that server no longer fits, authorized community
owners can move to another server without rebuilding the Community ID or member
relationships. Clients verify signed identities and events, and online members
can temporarily sync directly during a short server outage.

The protocol core, signed event history, local SQLite storage, single-person
invites, QUIC synchronization, replaceable Home Servers, and backup recovery are
implemented. A first native desktop client and permanent test community now
exist for a friends alpha; account recovery, multi-device support, and release
hardening are not complete.

## 它不是什么

- 不是“完全没有服务器”。长期社区仍然需要一台尽量在线的 Home Server。
- 不是“资源凭空免费”。机器、存储和带宽始终由某个人或组织承担。
- 不是把所有流量都强制 P2P。文字与短期故障可以利用直连，大规模语音和
  投屏以后仍可能需要 Relay、树形转发或 SFU。
- 不是用区块链决定社区治理。协议只提供身份、签名、权限和迁移能力，具体
  治理规则留给社区。
- 不是只服务游戏玩家。朋友群、兴趣社区、开源项目和组织协作都在目标范围。

## 与 Discord 最根本的区别

Discord 同时控制账号、社区入口、服务器和历史数据。ChatCommons 把这些
责任拆开：

```text
个人拥有签名身份
社区拥有治理权和稳定 Community ID
Home Server 提供在线、历史和连接服务
服务器运营者可以被替换
ChatCommons 官方不审批社区迁移
```

因此核心承诺不是“永不使用服务器”，而是：

> 服务器可以存在，但不能成为社区唯一且不可替换的权力中心。

## 为什么从小型朋友群开始

项目没有大型平台的服务器预算，因此第一阶段优先验证 2–10 人的长期社区：
文字聊天、邀请、离线历史、服务器短期故障和迁移。这既能控制基础设施成本，
也能验证真正独特的能力。语音、投屏和大社区扩展会在可靠文字社区之后进行。
