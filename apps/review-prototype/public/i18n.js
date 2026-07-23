(function () {
  'use strict';

  const STORAGE_KEY = 'chatcommons-locale';
  const englishText = {
    '交互原型': 'Interactive prototype',
    '项目介绍': 'About',
    '找社区、房间或操作': 'Find communities, rooms, or actions',
    '调整界面': 'Customize',
    '开源 · 社区自有 · 可迁移': 'Open source · Community-owned · Portable',
    '让社区聊天不再被单一平台锁住。': 'Community chat without platform lock-in.',
    'ChatCommons 是一个开源社区聊天应用。每个社区选择自己的长期在线主服务器；以后可以更换服务器，而不必重建身份、成员关系和整个社区。': 'ChatCommons is an open-source community chat app. Each community chooses its long-running home server and can move later without rebuilding identities, membership, or the community itself.',
    '了解它如何工作': 'See how it works',
    '下载桌面测试版': 'Download desktop alpha',
    '复制项目简介': 'Copy project brief',
    '项目原则': 'Project principles',
    '像普通群聊一样使用': 'Use it like an ordinary group chat',
    '技术细节默认隐藏，邀请后直接加入。': 'Technical details stay hidden. Join directly from an invite.',
    '社区选择由谁托管': 'The community chooses its host',
    '可以是朋友、社区、企业或托管商。': 'It can be a friend, community, company, or hosting provider.',
    '服务器可以被替换': 'The server can be replaced',
    '社区身份不绑定某一家公司或机器。': 'Community identity is not tied to one company or machine.',
    '网页是交互原型': 'This webpage is an interactive prototype',
    '桌面 alpha 已连接真实签名聊天；这里的页面消息仍是演示数据。': 'The desktop alpha runs real signed chat; messages on this webpage are still demo data.',
    '个人首页': 'Personal home',
    '星期一 · 7 月 20 日': 'Monday · July 20',
    '现在': 'Now',
    '先处理需要你注意的事，再回到最近的社区。': 'Handle what needs your attention, then return to a recent community.',
    '通过邀请加入': 'Join with invite',
    '创建社区': 'Create community',
    '继续': 'Continue',
    '回到最近的对话': 'Return to recent conversations',
    '全部社区': 'All communities',
    '周末游戏组 · 闲聊': 'Weekend Games · General',
    '今晚八点还是九点？': 'Eight or nine tonight?',
    '阿岚': 'Arlan',
    '刚刚 · 3 条新消息': 'Just now · 3 new messages',
    '开源小组 · 产品讨论': 'Open Source Group · Product',
    '首页不应该是一排服务器图标': 'The home screen should not be a rail of server icons',
    '小陈': 'Chen',
    '12 分钟前 · 1 条新消息': '12 min ago · 1 new message',
    '家里人 · 日常': 'Family · Everyday',
    '照片我晚点发到群里': 'I will share the photos later',
    '妈妈': 'Mom',
    '昨天 · 没有未读': 'Yesterday · All caught up',
    '动态': 'Activity',
    '值得回来的几件事': 'A few reasons to come back',
    '需要你注意': 'Needs your attention',
    '提及、未读和邀请': 'Mentions, unread messages, and invites',
    '小陈提到了你': 'Chen mentioned you',
    '开源小组': 'Open Source Group',
    '12 分钟': '12 min',
    '周末游戏组有新消息': 'New messages in Weekend Games',
    '周末游戏组': 'Weekend Games',
    '从你上次离开的位置继续': 'Continue from where you left off',
    '刚刚': 'Just now',
    '邀请一个朋友': 'Invite a friend',
    '每个链接只供一个人使用': 'Each link works for one person',
    '创建邀请': 'Create invite',
    '你的社区': 'Your communities',
    '选择一个社区继续': 'Choose a community to continue',
    '查找和筛选': 'Find and filter',
    '社区': 'Community',
    '3 人在线': '3 online',
    '邀请朋友': 'Invite friends',
    '闲聊': 'General',
    '游戏': 'Games',
    '装备讨论': 'Gear',
    '浏览房间': 'Browse rooms',
    '今天': 'Today',
    '今晚八点还是九点？我想先把新地图开一下。': 'Eight or nine tonight? I want to open the new map first.',
    '八点半吧，我回家以后正好能赶上。': 'Eight thirty works. I should be home just in time.',
    '你': 'You',
    '可以，我先开着房间等你们。': 'Sure. I will keep the room open while I wait.',
    '3 条新消息': '3 new messages',
    '木木': 'Mumu',
    '我也来。今晚可以顺便试一下新的聊天原型。': 'Count me in. We can try the new chat prototype tonight.',
    'Enter 发送 · Shift Enter 换行': 'Enter to send · Shift Enter for a new line',
    '发送': 'Send',
    '成员': 'Members',
    '现在在线': 'Online now',
    '正在浏览闲聊': 'Viewing General',
    '12 分钟前发言': 'Spoke 12 min ago',
    '刚刚发言': 'Spoke just now',
    '成员身份由自己的密钥证明。网络细节只在诊断页面出现。': 'Members prove their identity with their own keys. Network details only appear in diagnostics.',
    'ChatCommons 是什么': 'What is ChatCommons?',
    '它不是“不要服务器”，而是“不要唯一且不可替换的平台服务器”。': 'It is not about having no servers. It is about having no single, irreplaceable platform server.',
    '正常情况下，社区主服务器保存历史、接收消息并帮助成员连接。服务器短暂离线时，已在线成员可以临时直接同步；服务器恢复后再合并签名事件。': 'Normally, the community home server stores history, receives messages, and helps members connect. During a short outage, online members can temporarily sync directly and merge signed events when the server returns.',
    '我们已经完成': 'Implemented',
    '签名身份与事件、本地 SQLite 历史、单人邀请、QUIC 同步、可替换主服务器及备份恢复。': 'Signed identities and events, local SQLite history, single-person invites, QUIC sync, a replaceable home server, and backup recovery.',
    '现在可以测试': 'Ready to test now',
    '原生桌面 alpha 已接入协议内核，可以创建本机身份、用单人邀请加入永久测试社区并发送真实签名消息。': 'The native desktop alpha is connected to the protocol core. It creates a local identity, joins the permanent test community with a single-person invite, and sends real signed messages.',
    '查看源代码': 'View source code',
    '关闭介绍': 'Close introduction',
    '项目简介已复制，可以直接发给朋友': 'Project brief copied. You can send it directly to a friend.',
    '手动复制项目简介': 'Copy the project brief manually',
    '社区与房间': 'Communities and rooms',
    '输入名称进行筛选': 'Filter by name',
    '收藏': 'Favorites',
    '全部社区与房间': 'All communities and rooms',
    '文字房间': 'Text rooms',
    '搜索结果会在这里实时筛选；协议不会因为界面分组而改变。': 'Results filter here instantly; UI grouping does not change the protocol.',
    '取消': 'Cancel',
    '创建一个社区': 'Create a community',
    '只需要一个名字。身份、存储和连接方式会自动准备。': 'Just choose a name. Identity, storage, and connectivity are prepared automatically.',
    '社区名称': 'Community name',
    '例如：周五电影夜': 'For example: Friday Movie Night',
    '创建并进入': 'Create and enter',
    '粘贴朋友发来的邀请。客户端会自动寻找社区，不需要输入地址或端口。': 'Paste an invite from a friend. The client finds the community automatically—no address or port required.',
    '邀请链接': 'Invite link',
    '查看社区': 'Preview community',
    '邀请已经准备好': 'Your invite is ready',
    '这个原型无法读取剪贴板时，可以手动复制下面的内容。': 'If this prototype cannot access the clipboard, copy the invite below.',
    '单人邀请': 'Single-person invite',
    '完成': 'Done',
    '去任何地方': 'Go anywhere',
    '社区、房间和常用操作会出现在同一个搜索入口。': 'Communities, rooms, and common actions appear in one search.',
    '搜索': 'Search',
    '输入“闲聊”“邀请”或社区名称': 'Try “General,” “invite,” or a community name',
    '打开': 'Open',
    '视图': 'View',
    '默认布局无需配置。这里的选项只改变你的本地显示，不改变社区协议。': 'The default layout needs no setup. These options only change your local view, not the community protocol.',
    '主题': 'Theme',
    '明亮': 'Light',
    '夜间': 'Dark',
    '信息密度': 'Density',
    '舒适': 'Comfortable',
    '紧凑': 'Compact',
    '原型说明': 'About this prototype',
    '社区卡片、顶部房间和按需成员抽屉是本轮重点。请通过右下角“标注意见”直接点选不自然的地方。': 'This round focuses on community cards, top room tabs, and an on-demand member drawer. Use Annotate in the lower-right corner to flag anything that feels unnatural.',
    '原型评审': 'Prototype review',
    '正常操作页面；需要评论时再点“标注意见”。': 'Use the page normally. Click Annotate when you want to leave feedback.',
    '感谢每一位早期评审者': 'Thank you, early reviewers',
    '感谢你们愿意花时间评审 ChatCommons。关于项目说明、视觉层级、邀请、提及、导航和页面空白的意见都非常有用。我们已经根据这些反馈更新了原型，并补充了更清楚的产品介绍。': 'Thank you all for taking the time to review ChatCommons. Your comments about the project explanation, visual hierarchy, invitations, mentions, navigation, and empty space were genuinely useful. We have updated the prototype and added a clearer product brief based on your feedback.',
    '我们也希望把你列为早期产品与设计贡献者。请告诉我们你希望公开使用的名字或账号；如果更愿意匿名，也完全没问题。': 'We would also like to credit you as early product and design contributors. Please tell us which public name or handle you would like us to use—or if you would prefer to stay anonymous.',
    '贡献者署名': 'Contributor credit',
    '谢谢你帮助改进 ChatCommons。提交后会进入管理员收件箱，确认后再加入公开贡献者名单。': 'Thank you for helping improve ChatCommons. Your preference will go to the owner inbox and will only be added to the public contributor list after confirmation.',
    '我希望保持匿名': 'I prefer to remain anonymous',
    '公开名称或账号': 'Public name or handle',
    '例如：Pinksie 或 @pinksie': 'For example: Pinksie or @pinksie',
    '个人主页（可选）': 'Profile link (optional)',
    '提交署名信息': 'Submit credit preference',
    '标注意见': 'Annotate',
    '取消标注': 'Cancel annotation',
    '已有意见': 'Feedback',
    '复制审阅链接': 'Copy review link',
    '还没有提交意见。': 'No feedback yet.',
    '布局': 'Layout',
    '文案': 'Copy',
    '交互': 'Interaction',
    '产品逻辑': 'Product logic',
    '待确认': 'Pending',
    '处理中': 'In progress',
    '待验收': 'Ready for review',
    '已完成': 'Completed',
    '暂不处理': 'Not planned',
    '已撤回': 'Withdrawn',
    '这里需要怎么改？': 'What should change here?',
    '编辑意见': 'Edit feedback',
    '意见类型': 'Category',
    '优先级': 'Priority',
    '一般': 'Normal',
    '重要': 'High',
    '不急': 'Low',
    '具体意见': 'Feedback',
    '提交': 'Submit',
    '保存修改': 'Save changes',
    '回到现在': 'Back to Now',
    '全局操作': 'Global actions',
    '打开个人资料': 'Open profile',
    '查看在线成员': 'View online members',
    '房间': 'Rooms',
    '消息': 'Messages',
    '房间信息': 'Room details',
    '更多操作': 'More actions',
    '发送消息到当前房间': 'Send a message to the current room',
    '关闭成员列表': 'Close member list',
    '关闭': 'Close',
    '请输入具体意见': 'Enter your feedback',
    '直接说你的感觉，例如：我不知道这里点了会发生什么': 'Say what you feel—for example: I do not know what will happen when I click this.',
    '你选择了：': 'You selected:',
    '页面上的这个位置': 'this part of the page',
    '回复：': 'Reply:',
    '意见已提交': 'Feedback submitted',
    '评审服务暂时不可用': 'The review service is temporarily unavailable',
    '周': 'W',
    '开': 'O',
    '家': 'F',
    '岚': 'A',
    '陈': 'C',
    '木': 'M',
    '宇': 'Y',
    '妈': 'M',
    '林': 'L',
  };
  const chineseText = new Map(Object.entries(englishText).map(([zh, en]) => [en, zh]));
  const originalText = new WeakMap();
  const originalAttributes = new WeakMap();
  let locale = readLocale();

  function readLocale() {
    try {
      return localStorage.getItem(STORAGE_KEY) === 'en' ? 'en' : 'zh-CN';
    } catch (_) {
      return 'zh-CN';
    }
  }

  function pick(chinese, english) {
    return locale === 'en' ? english : chinese;
  }

  function sourceValue(value) {
    return chineseText.get(value) || value;
  }

  function translateTextNode(node) {
    const current = node.nodeValue || '';
    const core = current.trim();
    if (!core) return;
    if (!originalText.has(node)) originalText.set(node, sourceValue(core));
    const source = originalText.get(node);
    const translated = locale === 'en' ? (englishText[source] || source) : source;
    node.nodeValue = current.slice(0, current.indexOf(core)) + translated + current.slice(current.indexOf(core) + core.length);
  }

  function translateElement(element) {
    const attributes = ['aria-label', 'placeholder', 'title'];
    let originals = originalAttributes.get(element);
    if (!originals) {
      originals = {};
      originalAttributes.set(element, originals);
    }
    attributes.forEach((name) => {
      if (!element.hasAttribute(name)) return;
      if (!(name in originals)) originals[name] = sourceValue(element.getAttribute(name));
      const source = originals[name];
      element.setAttribute(name, locale === 'en' ? (englishText[source] || source) : source);
    });
  }

  function translateSubtree(root) {
    if (!root) return;
    if (root.nodeType === Node.TEXT_NODE) {
      translateTextNode(root);
      return;
    }
    if (root.nodeType !== Node.ELEMENT_NODE && root.nodeType !== Node.DOCUMENT_NODE) return;
    if (root.nodeType === Node.ELEMENT_NODE) translateElement(root);
    root.querySelectorAll?.('*').forEach(translateElement);
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        return node.parentElement?.closest('script, style, textarea, [data-no-i18n]')
          ? NodeFilter.FILTER_REJECT
          : NodeFilter.FILTER_ACCEPT;
      },
    });
    while (walker.nextNode()) translateTextNode(walker.currentNode);
  }

  function updateLanguageControl() {
    const button = document.querySelector('#language-toggle');
    if (!button) return;
    button.textContent = locale === 'en' ? '中文' : 'EN';
    button.setAttribute('aria-label', locale === 'en' ? '切换到中文' : 'Switch to English');
  }

  function setLocale(nextLocale, persist = true) {
    locale = nextLocale === 'en' ? 'en' : 'zh-CN';
    if (persist) {
      try { localStorage.setItem(STORAGE_KEY, locale); } catch (_) { /* Local storage may be disabled. */ }
    }
    document.documentElement.lang = locale;
    document.documentElement.style.setProperty('--featured-label', locale === 'en' ? '"Tonight"' : '"今晚"');
    const description = document.querySelector('meta[name="description"]');
    if (description) description.content = pick(
      'ChatCommons 是一个开源、社区自有、服务器可迁移的聊天应用原型。',
      'ChatCommons is an open-source, community-owned chat prototype with replaceable servers.'
    );
    translateSubtree(document);
    updateLanguageControl();
    window.dispatchEvent(new CustomEvent('chatcommons:locale-change', { detail: { locale } }));
  }

  window.chatcommonsI18n = {
    get locale() { return locale; },
    pick,
    canonicalText: sourceValue,
    setLocale,
    toggle() { setLocale(locale === 'en' ? 'zh-CN' : 'en'); },
    translateSubtree,
  };

  setLocale(locale, false);
  new MutationObserver((records) => {
    if (locale !== 'en') return;
    records.forEach((record) => record.addedNodes.forEach(translateSubtree));
  }).observe(document.body, { childList: true, subtree: true });
}());
