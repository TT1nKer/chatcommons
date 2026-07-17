# China launch legal and operational checklist

Status: questions for qualified PRC counsel; not legal advice

The architecture cannot determine legal classification by itself. Before any
public beta or operated infrastructure, counsel should classify the company for
each concrete service: client distribution, account service, invitation links,
bootstrap, relay, offline mailbox, push, directory, file storage, voice/SFU and
payments.

## Confirm with counsel

- whether each service makes the operator a network operator, internet information
  service provider, or other regulated service provider;
- the real-identity requirements applicable to instant messaging and how protocol
  public-key identity can be separated from verification held by an official
  service;
- ICP filing/licensing and any telecom-service classification;
- network security graded-protection obligations and incident reporting;
- prohibited-content handling, record preservation, reporting, user complaints
  and appeals;
- encryption and cryptography compliance for the intended deployment;
- personal-information role, lawful basis, minimization, retention, deletion,
  sensitive information and cross-border transfer for every official service;
- obligations involving minors, voice, public communities, recommendations,
  attachments, payments and third-party bots;
- lawful-request intake, authority verification, scope control, disclosure logging
  and confidentiality requirements;
- responsibilities and contract boundaries for third-party/self-hosted nodes.

## Engineering evidence required for review

- a data-flow diagram and data inventory per service;
- exact plaintext/metadata visibility at every hop;
- retention and deletion behavior, including backups;
- identity verification and protocol-key mapping proposal;
- moderation control matrix and incident-response runbook;
- proposed terms, privacy notice, acceptable-use policy and appeal procedure;
- deployment regions and every expected cross-border data flow;
- list of statements the product makes about privacy, deletion and availability.

## Official source baseline

- The amended [Cybersecurity Law](https://www.cac.gov.cn/2025-12/29/c_1768735112911946.htm)
  includes duties relevant to network operation and states a real-identity
  requirement when providing instant-messaging services. Applicability to each
  ChatCommons component requires counsel's analysis.
- The [Personal Information Protection Law](https://www.cac.gov.cn/2021-08/20/c_1631050028355286.htm)
  governs personal-information processing and makes data minimization and explicit
  service-by-service inventories necessary.
- The [Provisions on the Governance of the Online Information Content Ecosystem](https://www.cac.gov.cn/2019-12/20/c_1578375159509309.htm)
  describe platform content-governance mechanisms, records and reporting duties.
- The [Provisions on Governance of Cyber Violence Information](https://www.cac.gov.cn/2024-06/14/c_1720043894161555.htm)
  add specific governance requirements relevant to harassment and reporting.
- A July 2026 [draft revision of the Internet Information Services Measures](https://www.cac.gov.cn/2026-07/03/c_1784822399677167.htm)
  is under public consultation as of this checklist. It is a watch item, not
  treated here as effective law.

This list is intentionally incomplete. Product launch remains blocked until a
lawyer maps the actual deployment and business model to current requirements.
