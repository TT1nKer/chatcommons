(function () {
  'use strict';

  const $ = (selector, parent = document) => parent.querySelector(selector);
  const $$ = (selector, parent = document) => [...parent.querySelectorAll(selector)];
  const i18n = window.chatcommonsI18n;
  const l = (chinese, english) => i18n.pick(chinese, english);
  const densityStorageKey = 'chatcommons-density';
  const state = { community: 'weekend', room: 'general', screen: 'home' };
  const rooms = {
    general: { zh: '闲聊', en: 'General' },
    games: { zh: '游戏', en: 'Games' },
    gear: { zh: '装备讨论', en: 'Gear' },
  };
  const communities = {
    weekend: { zh: '周末游戏组', en: 'Weekend Games', symbolZh: '周', symbolEn: 'W', symbolClass: 'symbol-coral' },
    opensource: { zh: '开源小组', en: 'Open Source Group', symbolZh: '开', symbolEn: 'O', symbolClass: 'symbol-blue' },
    family: { zh: '家里人', en: 'Family', symbolZh: '家', symbolEn: 'F', symbolClass: 'symbol-green' },
  };

  function storedDensity() {
    try { return localStorage.getItem(densityStorageKey) === 'compact' ? 'compact' : 'comfortable'; }
    catch (_) { return 'comfortable'; }
  }

  function applyDensity(value, persist = true) {
    const density = value === 'compact' ? 'compact' : 'comfortable';
    $('#app-shell').dataset.density = density;
    if (persist) {
      try { localStorage.setItem(densityStorageKey, density); }
      catch (_) { /* The selected density still applies for this session. */ }
    }
  }

  applyDensity(storedDensity(), false);

  function roomName() {
    const room = rooms[state.room] || rooms.general;
    return l(room.zh, room.en);
  }

  function communityName() {
    const community = communities[state.community];
    return l(community.zh, community.en);
  }

  function refreshLocalizedState() {
    const community = communities[state.community];
    $('#community-title').textContent = communityName();
    const symbol = $('#community-symbol');
    symbol.textContent = l(community.symbolZh, community.symbolEn);
    symbol.className = 'community-symbol ' + community.symbolClass;
    $('#room-title').textContent = roomName();
    $('#message-input').placeholder = l(
      '在' + roomName() + '中说点什么……',
      'Message #' + roomName().toLowerCase() + '…'
    );
    document.title = state.screen === 'home'
      ? l('ChatCommons · 现在', 'ChatCommons · Now')
      : communityName() + ' · ' + roomName();
    document.documentElement.dataset.reviewScreen = state.screen === 'home'
      ? 'home'
      : 'community:' + state.community + ':room:' + state.room;
    window.dispatchEvent(new Event('chatcommons:screen-change'));
  }

  function showScreen(name) {
    state.screen = name === 'community' ? 'community' : 'home';
    const home = $('#home-screen');
    const community = $('#community-screen');
    home.hidden = state.screen !== 'home';
    community.hidden = state.screen !== 'community';
    refreshLocalizedState();
    window.scrollTo({ top: 0, behavior: 'smooth' });
  }

  function openCommunity(id) {
    state.community = communities[id] ? id : 'weekend';
    showScreen('community');
  }

  function setRoom(button) {
    $$('.room-tab').forEach((tab) => {
      const active = tab === button;
      tab.classList.toggle('is-active', active);
      tab.setAttribute('aria-selected', String(active));
    });
    state.room = rooms[button.dataset.room] ? button.dataset.room : 'general';
    refreshLocalizedState();
    toast(l('已切换到 ' + roomName(), 'Switched to ' + roomName()));
  }

  function toast(message) {
    const node = $('#app-toast');
    node.textContent = message;
    node.classList.add('show');
    clearTimeout(toast.timer);
    toast.timer = setTimeout(() => node.classList.remove('show'), 2400);
  }

  function closeDialog() {
    const layer = $('#dialog-layer');
    layer.hidden = true;
    layer.innerHTML = '';
  }

  function openDialog({ title, description, label, placeholder, action, submitText }) {
    const layer = $('#dialog-layer');
    layer.innerHTML = `<form class="dialog" data-dialog-form>
      <h2>${title}</h2>
      <p>${description}</p>
      <label>${label}<input name="value" autocomplete="off" maxlength="180" placeholder="${placeholder}" required /></label>
      <div class="dialog-actions">
        <button class="secondary-action" type="button" data-dialog-close>取消</button>
        <button class="primary-action" type="submit">${submitText}</button>
      </div>
    </form>`;
    i18n.translateSubtree(layer);
    layer.hidden = false;
    $('[data-dialog-close]', layer).onclick = closeDialog;
    layer.onclick = (event) => { if (event.target === layer) closeDialog(); };
    $('[data-dialog-form]', layer).onsubmit = (event) => {
      event.preventDefault();
      const value = event.currentTarget.elements.value.value.trim();
      if (!value) return;
      closeDialog();
      action(value);
    };
    setTimeout(() => $('input', layer).focus(), 0);
  }

  function openCreate() {
    openDialog({
      title: '创建一个社区',
      description: '只需要一个名字。身份、存储和连接方式会自动准备。',
      label: '社区名称',
      placeholder: '例如：周五电影夜',
      submitText: '创建并进入',
      action: (value) => toast(l('“' + value + '”已创建（原型演示）', '“' + value + '” created (prototype)')),
    });
  }

  function openJoin() {
    openDialog({
      title: '通过邀请加入',
      description: '粘贴朋友发来的邀请。客户端会自动寻找社区，不需要输入地址或端口。',
      label: '邀请链接',
      placeholder: 'chatcommons://invite/…',
      submitText: '查看社区',
      action: () => toast(l('邀请有效，正在准备加入（原型演示）', 'Invite accepted. Preparing to join (prototype)')),
    });
  }

  async function copyInvite() {
    const invite = 'chatcommons://invite/preview-single-use-example';
    try {
      await navigator.clipboard.writeText(invite);
      toast(l('单人邀请已复制，使用一次后失效', 'Single-person invite copied. It expires after use.'));
    } catch (_) {
      openDialog({
        title: '邀请已经准备好',
        description: '这个原型无法读取剪贴板时，可以手动复制下面的内容。',
        label: '单人邀请',
        placeholder: invite,
        submitText: '完成',
        action: () => toast(l('已关闭邀请', 'Invite closed')),
      });
      const input = $('#dialog-layer input');
      input.value = invite;
      input.select();
    }
  }

  function openSearch() {
    openDialog({
      title: '去任何地方',
      description: '社区、房间和常用操作会出现在同一个搜索入口。',
      label: '搜索',
      placeholder: '输入“闲聊”“邀请”或社区名称',
      submitText: '打开',
      action: (value) => toast(l('正在查找“' + value + '”（原型演示）', 'Searching for “' + value + '” (prototype)')),
    });
  }

  function mountPanel(markup) {
    const panel = $('#side-panel');
    panel.innerHTML = markup;
    i18n.translateSubtree(panel);
    panel.hidden = false;
    $('[data-panel-close]', panel).onclick = () => { panel.hidden = true; };
    panel.onclick = (event) => { if (event.target === panel) panel.hidden = true; };
    return panel;
  }

  function openAbout() {
    mountPanel(`<section class="panel-card" role="dialog" aria-modal="true" aria-labelledby="about-title">
      <div class="drawer-heading"><div><p>项目介绍</p><h2 id="about-title">ChatCommons 是什么</h2></div><button class="icon-button" type="button" data-panel-close aria-label="关闭介绍">×</button></div>
      <p>它不是“不要服务器”，而是“不要唯一且不可替换的平台服务器”。</p>
      <div class="about-flow">
        <div class="about-step"><span>01</span><div><strong>社区选择由谁托管</strong><small>正常情况下，社区主服务器保存历史、接收消息并帮助成员连接。服务器短暂离线时，已在线成员可以临时直接同步；服务器恢复后再合并签名事件。</small></div></div>
        <div class="about-step"><span>02</span><div><strong>我们已经完成</strong><small>签名身份与事件、本地 SQLite 历史、单人邀请、QUIC 同步、可替换主服务器及备份恢复。</small></div></div>
        <div class="about-step"><span>03</span><div><strong>现在可以测试</strong><small>原生桌面 alpha 已接入协议内核，可以创建本机身份、用单人邀请加入永久测试社区并发送真实签名消息。</small></div></div>
      </div>
      <div class="panel-links"><a href="https://github.com/TT1nKer/chatcommons/releases/tag/v0.1.0-alpha.1" target="_blank" rel="noopener noreferrer">下载桌面测试版</a><a href="https://github.com/TT1nKer/chatcommons" target="_blank" rel="noopener noreferrer">查看源代码</a><button class="secondary-action" type="button" data-copy-brief>复制项目简介</button></div>
    </section>`);
    $('[data-copy-brief]', $('#side-panel')).onclick = copyProjectBrief;
  }

  async function copyProjectBrief() {
    const brief = l(
      'ChatCommons 是一个开源社区聊天应用，目标是提供熟悉、顺畅的社区体验，同时避免所有社区被同一个平台锁住。每个社区选择自己的长期在线主服务器；以后可以迁移服务器，而不必重建身份、成员关系和整个社区。成员身份和事件由签名验证，服务器短暂离线时，已在线成员可以临时直接同步。协议内核、单人邀请、QUIC 同步、可替换主服务器、备份恢复和第一版原生桌面客户端已经实现，正在进行朋友 alpha 测试。\n\nhttps://ttinker.net/chatcommons/',
      'ChatCommons is an open-source community chat app designed to feel familiar without locking every community into one platform. Each community chooses a long-running home server and can move later without rebuilding identities, membership, or the community itself. Signed identities and events are verified by clients, while online members can temporarily sync directly during a short server outage. The protocol core, single-person invites, QUIC sync, replaceable home server, backup recovery, and the first native desktop client are implemented and entering friends-alpha testing.\n\nhttps://ttinker.net/chatcommons/'
    );
    try {
      await navigator.clipboard.writeText(brief);
      toast(l('项目简介已复制，可以直接发给朋友', 'Project brief copied. You can send it directly to a friend.'));
    } catch (_) {
      window.prompt(l('手动复制项目简介', 'Copy the project brief manually'), brief);
    }
  }

  function openCommunityBrowser(roomsOnly = false) {
    const panel = mountPanel(`<section class="panel-card" role="dialog" aria-modal="true" aria-labelledby="browser-title">
      <div class="drawer-heading"><div><p>社区与房间</p><h2 id="browser-title">查找和筛选</h2></div><button class="icon-button" type="button" data-panel-close aria-label="关闭">×</button></div>
      <p>搜索结果会在这里实时筛选；协议不会因为界面分组而改变。</p>
      <input class="browser-search" type="search" autocomplete="off" placeholder="输入名称进行筛选" aria-label="输入名称进行筛选" />
      <div class="browser-group" ${roomsOnly ? 'hidden' : ''}><strong>收藏</strong><div class="browser-list">
        <button type="button" data-browser-community="weekend"><strong>周末游戏组</strong><small>闲聊 · 3 条新消息</small></button>
        <button type="button" data-browser-community="opensource"><strong>开源小组</strong><small>产品讨论 · 提到了你</small></button>
      </div></div>
      <div class="browser-group"><strong>${roomsOnly ? '文字房间' : '全部社区与房间'}</strong><div class="browser-list">
        ${roomsOnly ? '' : '<button type="button" data-browser-community="family"><strong>家里人</strong><small>日常 · 没有未读</small></button>'}
        <button type="button" data-browser-room="general"><strong>闲聊</strong><small>文字房间</small></button>
        <button type="button" data-browser-room="games"><strong>游戏</strong><small>文字房间</small></button>
        <button type="button" data-browser-room="gear"><strong>装备讨论</strong><small>文字房间</small></button>
      </div></div>
    </section>`);
    const input = $('.browser-search', panel);
    input.oninput = () => {
      const query = input.value.trim().toLocaleLowerCase();
      $$('.browser-list button', panel).forEach((button) => {
        button.hidden = Boolean(query) && !button.textContent.toLocaleLowerCase().includes(query);
      });
    };
    $$('[data-browser-community]', panel).forEach((button) => {
      button.onclick = () => { panel.hidden = true; openCommunity(button.dataset.browserCommunity); };
    });
    $$('[data-browser-room]', panel).forEach((button) => {
      button.onclick = () => {
        const tab = $(`[data-room="${button.dataset.browserRoom}"]`);
        panel.hidden = true;
        if (state.screen !== 'community') openCommunity(state.community);
        if (tab) setRoom(tab);
      };
    });
    input.focus();
  }

  function openPanel() {
    const panel = mountPanel(`<section class="panel-card" role="dialog" aria-modal="true" aria-labelledby="customize-title">
      <div class="drawer-heading"><div><p>视图</p><h2 id="customize-title">调整界面</h2></div><button class="icon-button" type="button" data-panel-close aria-label="关闭">×</button></div>
      <p>默认布局无需配置。这里的选项只改变你的本地显示，不改变社区协议。</p>
      <div class="setting-group"><h3>主题</h3><div class="setting-options"><button type="button" data-theme-choice="light">明亮</button><button type="button" data-theme-choice="dark">夜间</button></div></div>
      <div class="setting-group"><h3>信息密度</h3><div class="setting-options"><button type="button" data-density-choice="comfortable" class="is-active">舒适</button><button type="button" data-density-choice="compact">紧凑</button></div></div>
      <div class="setting-group"><h3>原型说明</h3><p>社区卡片、顶部房间和按需成员抽屉是本轮重点。请通过右下角“标注意见”直接点选不自然的地方。</p></div>
    </section>`);
    $$('[data-theme-choice]', panel).forEach((button) => {
      button.classList.toggle('is-active', document.documentElement.dataset.theme === button.dataset.themeChoice);
      button.onclick = () => {
        document.documentElement.dataset.theme = button.dataset.themeChoice;
        $$('[data-theme-choice]', panel).forEach((item) => item.classList.toggle('is-active', item === button));
      };
    });
    $$('[data-density-choice]', panel).forEach((button) => {
      button.classList.toggle('is-active', $('#app-shell').dataset.density === button.dataset.densityChoice);
      button.onclick = () => {
        applyDensity(button.dataset.densityChoice);
        $$('[data-density-choice]', panel).forEach((item) => item.classList.toggle('is-active', item === button));
      };
    });
  }

  function toggleMembers(show) {
    $('#member-drawer').hidden = !show;
  }

  function sendMessage(event) {
    event.preventDefault();
    const input = $('#message-input');
    const text = input.value.trim();
    if (!text) return;
    const article = document.createElement('article');
    article.className = 'message message-self';
    const avatar = document.createElement('span');
    avatar.className = 'message-avatar self';
    avatar.textContent = l('林', 'L');
    const body = document.createElement('div');
    const header = document.createElement('header');
    const author = document.createElement('strong');
    author.textContent = l('你', 'You');
    const time = document.createElement('time');
    time.textContent = l('刚刚', 'Just now');
    const content = document.createElement('p');
    content.dataset.noI18n = 'true';
    content.textContent = text;
    header.append(author, time);
    body.append(header, content);
    article.append(avatar, body);
    $('#message-list').appendChild(article);
    input.value = '';
    article.scrollIntoView({ behavior: 'smooth', block: 'center' });
    toast(l('消息已保存在本机（原型演示）', 'Message saved on this device (prototype)'));
  }

  document.addEventListener('click', (event) => {
    const community = event.target.closest('[data-community]');
    if (community) { openCommunity(community.dataset.community); return; }
    const room = event.target.closest('[data-room]');
    if (room) { setRoom(room); return; }
    const action = event.target.closest('[data-action]')?.dataset.action;
    if (!action) return;
    const actions = {
      home: () => showScreen('home'),
      create: openCreate,
      join: openJoin,
      invite: copyInvite,
      search: openSearch,
      about: openAbout,
      'copy-brief': copyProjectBrief,
      customize: openPanel,
      'toggle-language': () => i18n.toggle(),
      members: () => toggleMembers(true),
      'close-members': () => toggleMembers(false),
      rooms: () => openCommunityBrowser(true),
      profile: () => toast(l('个人资料：林间（原型演示）', 'Profile: Linjian (prototype)')),
      'all-communities': () => openCommunityBrowser(false),
      'room-info': () => toast(roomName() + l('：社区里的持久文字房间', ': a persistent text room in this community')),
      'future-feature': () => toast(l('文件、语音和投屏尚未进入本轮原型', 'Files, voice, and screen sharing are not in this prototype yet')),
    };
    actions[action]?.();
  });

  $('#composer').addEventListener('submit', sendMessage);
  $('#message-input').addEventListener('keydown', (event) => {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      $('#composer').requestSubmit();
    }
  });
  document.addEventListener('keydown', (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      openSearch();
    }
    if (event.key === 'Escape') {
      closeDialog();
      $('#side-panel').hidden = true;
      toggleMembers(false);
    }
  });
  window.addEventListener('chatcommons:locale-change', refreshLocalizedState);
  refreshLocalizedState();
}());
