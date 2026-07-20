(function () {
  'use strict';

  const $ = (selector, parent = document) => parent.querySelector(selector);
  const $$ = (selector, parent = document) => [...parent.querySelectorAll(selector)];
  const i18n = window.chatcommonsI18n;
  const l = (chinese, english) => i18n.pick(chinese, english);
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

  function openPanel() {
    const panel = $('#side-panel');
    panel.innerHTML = `<section class="panel-card" role="dialog" aria-modal="true" aria-labelledby="customize-title">
      <div class="drawer-heading"><div><p>视图</p><h2 id="customize-title">调整界面</h2></div><button class="icon-button" type="button" data-panel-close aria-label="关闭">×</button></div>
      <p>默认布局无需配置。这里的选项只改变你的本地显示，不改变社区协议。</p>
      <div class="setting-group"><h3>主题</h3><div class="setting-options"><button type="button" data-theme-choice="light">明亮</button><button type="button" data-theme-choice="dark">夜间</button></div></div>
      <div class="setting-group"><h3>信息密度</h3><div class="setting-options"><button type="button" data-density-choice="comfortable" class="is-active">舒适</button><button type="button" data-density-choice="compact">紧凑</button></div></div>
      <div class="setting-group"><h3>原型说明</h3><p>社区卡片、顶部房间和按需成员抽屉是本轮重点。请通过右下角“标注意见”直接点选不自然的地方。</p></div>
    </section>`;
    i18n.translateSubtree(panel);
    panel.hidden = false;
    $('[data-panel-close]', panel).onclick = () => { panel.hidden = true; };
    panel.onclick = (event) => { if (event.target === panel) panel.hidden = true; };
    $$('[data-theme-choice]', panel).forEach((button) => {
      button.classList.toggle('is-active', document.documentElement.dataset.theme === button.dataset.themeChoice);
      button.onclick = () => {
        document.documentElement.dataset.theme = button.dataset.themeChoice;
        $$('[data-theme-choice]', panel).forEach((item) => item.classList.toggle('is-active', item === button));
      };
    });
    $$('[data-density-choice]', panel).forEach((button) => {
      button.onclick = () => {
        $('#app-shell').dataset.density = button.dataset.densityChoice;
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
      customize: openPanel,
      'toggle-language': () => i18n.toggle(),
      members: () => toggleMembers(true),
      'close-members': () => toggleMembers(false),
      rooms: () => toast(l('房间较多时，这里会打开搜索面板', 'When there are more rooms, this opens a search panel')),
      profile: () => toast(l('个人资料：林间（原型演示）', 'Profile: Linjian (prototype)')),
      'all-communities': () => toast(l('当前共有 3 个社区', 'You currently have 3 communities')),
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
