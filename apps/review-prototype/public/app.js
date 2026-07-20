(function () {
  'use strict';

  const $ = (selector, parent = document) => parent.querySelector(selector);
  const $$ = (selector, parent = document) => [...parent.querySelectorAll(selector)];
  const state = { community: 'weekend', room: '闲聊' };
  const communities = {
    weekend: { name: '周末游戏组', symbol: '周', symbolClass: 'symbol-coral' },
    opensource: { name: '开源小组', symbol: '开', symbolClass: 'symbol-blue' },
    family: { name: '家里人', symbol: '家', symbolClass: 'symbol-green' },
  };

  function showScreen(name) {
    const home = $('#home-screen');
    const community = $('#community-screen');
    home.hidden = name !== 'home';
    community.hidden = name !== 'community';
    document.title = name === 'home' ? 'kaiyuan · 现在' : `${communities[state.community].name} · ${state.room}`;
    window.scrollTo({ top: 0, behavior: 'smooth' });
    window.dispatchEvent(new Event('chatcommons:screen-change'));
  }

  function openCommunity(id) {
    state.community = communities[id] ? id : 'weekend';
    const community = communities[state.community];
    $('#community-title').textContent = community.name;
    const symbol = $('#community-symbol');
    symbol.textContent = community.symbol;
    symbol.className = `community-symbol ${community.symbolClass}`;
    showScreen('community');
  }

  function setRoom(button) {
    $$('.room-tab').forEach((tab) => {
      const active = tab === button;
      tab.classList.toggle('is-active', active);
      tab.setAttribute('aria-selected', String(active));
    });
    state.room = button.dataset.room;
    $('#room-title').textContent = state.room;
    $('#message-input').placeholder = `在${state.room}中说点什么……`;
    document.title = `${communities[state.community].name} · ${state.room}`;
    window.dispatchEvent(new Event('chatcommons:screen-change'));
    toast(`已切换到 ${state.room}`);
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
      action: (value) => toast(`“${value}”已创建（原型演示）`),
    });
  }

  function openJoin() {
    openDialog({
      title: '通过邀请加入',
      description: '粘贴朋友发来的邀请。客户端会自动寻找社区，不需要输入地址或端口。',
      label: '邀请链接',
      placeholder: 'kaiyuan://invite/…',
      submitText: '查看社区',
      action: () => toast('邀请有效，正在准备加入（原型演示）'),
    });
  }

  async function copyInvite() {
    const invite = 'kaiyuan://invite/preview-single-use-example';
    try {
      await navigator.clipboard.writeText(invite);
      toast('单人邀请已复制，使用一次后失效');
    } catch (_) {
      openDialog({
        title: '邀请已经准备好',
        description: '这个原型无法读取剪贴板时，可以手动复制下面的内容。',
        label: '单人邀请',
        placeholder: invite,
        submitText: '完成',
        action: () => toast('已关闭邀请'),
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
      action: (value) => toast(`正在查找“${value}”（原型演示）`),
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
    avatar.textContent = '林';
    const body = document.createElement('div');
    const header = document.createElement('header');
    const author = document.createElement('strong');
    author.textContent = '你';
    const time = document.createElement('time');
    time.textContent = '刚刚';
    const content = document.createElement('p');
    content.textContent = text;
    header.append(author, time);
    body.append(header, content);
    article.append(avatar, body);
    $('#message-list').appendChild(article);
    input.value = '';
    article.scrollIntoView({ behavior: 'smooth', block: 'center' });
    toast('消息已保存在本机（原型演示）');
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
      members: () => toggleMembers(true),
      'close-members': () => toggleMembers(false),
      rooms: () => toast('房间较多时，这里会打开搜索面板'),
      profile: () => toast('个人资料：林间（原型演示）'),
      'all-communities': () => toast('当前共有 3 个社区'),
      'room-info': () => toast(`${state.room}：社区里的持久文字房间`),
      'future-feature': () => toast('文件、语音和投屏尚未进入本轮原型'),
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
}());
