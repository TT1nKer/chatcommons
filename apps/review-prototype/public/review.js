(function () {
  'use strict';
  const params = new URLSearchParams(location.search);
  const incoming = params.get('review');
  if (incoming) {
    sessionStorage.setItem('chatcommons-review-token', incoming);
    params.delete('review');
    const query = params.toString();
    history.replaceState({}, document.title, `${location.pathname}${query ? `?${query}` : ''}${location.hash}`);
  }
  const token = sessionStorage.getItem('chatcommons-review-token');
  if (!token || token.length < 40) return;

  const $ = (selector, parent = document) => parent.querySelector(selector);
  const $$ = (selector, parent = document) => [...parent.querySelectorAll(selector)];
  const l = (chinese, english) => window.chatcommonsI18n.pick(chinese, english);
  const state = { selecting: false, highlighted: null, reviews: [] };
  const statuses = {
    pending: ['待确认', 'Pending'],
    in_progress: ['处理中', 'In progress'],
    client_review: ['待验收', 'Ready for review'],
    completed: ['已完成', 'Completed'],
    rejected: ['暂不处理', 'Not planned'],
  };
  const categories = {
    layout: ['布局', 'Layout'],
    copy: ['文案', 'Copy'],
    feature: ['交互', 'Interaction'],
    product: ['产品逻辑', 'Product logic'],
  };
  const statusText = (status) => statuses[status] ? l(...statuses[status]) : status;
  const categoryText = (category) => categories[category] ? l(...categories[category]) : category;

  async function api(path, options = {}) {
    const response = await fetch(`./api${path}`, {
      credentials: 'same-origin',
      ...options,
      headers: { 'Content-Type': 'application/json', 'X-Review-Token': token, ...(options.headers || {}) },
    });
    const body = await response.json().catch(() => ({}));
    if (!response.ok) {
      const message = window.chatcommonsI18n.locale === 'en'
        ? 'The review service could not complete this request.'
        : (body.error || '评审服务暂时不可用');
      throw new Error(message);
    }
    return body;
  }

  function currentScreen() {
    return document.documentElement.dataset.reviewScreen || 'home';
  }

  function selectorFor(element) {
    if (!element || element === document.body) return 'body';
    const parts = [];
    let node = element;
    while (node && node.nodeType === 1 && node !== document.body && parts.length < 4) {
      let part = node.tagName.toLowerCase();
      if (node.id) {
        part += `#${node.id.replace(/[^a-zA-Z0-9_-]/g, '')}`;
        parts.unshift(part);
        break;
      }
      const classes = [...node.classList].filter((name) => !name.startsWith('review-') && name !== 'is-active').slice(0, 2);
      if (classes.length) part += `.${classes.map((name) => name.replace(/[^a-zA-Z0-9_-]/g, '')).join('.')}`;
      parts.unshift(part);
      node = node.parentElement;
    }
    return parts.join(' > ') || 'body';
  }

  function visibleText(element) {
    return (element.innerText || element.getAttribute('aria-label') || element.tagName || '').trim().replace(/\s+/g, ' ').slice(0, 240);
  }

  function clearHighlight() {
    if (state.highlighted) state.highlighted.classList.remove('review-highlight');
    state.highlighted = null;
  }

  function setSelecting(value) {
    state.selecting = value;
    document.body.classList.toggle('review-selecting', value);
    $('[data-review-select]').textContent = value ? l('取消标注', 'Cancel annotation') : l('标注意见', 'Annotate');
    if (!value) clearHighlight();
  }

  function notify(message) {
    let node = $('#review-notice');
    if (!node) {
      node = document.createElement('div');
      node.id = 'review-notice';
      node.className = 'review-notice';
      node.dataset.reviewUi = 'true';
      document.body.appendChild(node);
    }
    node.textContent = message;
    node.classList.add('show');
    clearTimeout(notify.timer);
    notify.timer = setTimeout(() => node.classList.remove('show'), 2600);
  }

  function renderList() {
    const list = $('#review-list');
    list.replaceChildren();
    if (!state.reviews.length) {
      const empty = document.createElement('small');
      empty.textContent = l('还没有提交意见。', 'No feedback yet.');
      list.appendChild(empty);
      return;
    }
    state.reviews.forEach((item) => {
      const row = document.createElement('div');
      row.className = 'review-item';
      const title = document.createElement('strong');
      title.textContent = categoryText(item.category) + ' · ' + item.message;
      const context = document.createElement('small');
      context.textContent = (item.targetText || item.screen) + ' · ' + statusText(item.status);
      row.append(title, context);
      if (item.adminReply) {
        const reply = document.createElement('small');
        reply.textContent = l('回复：', 'Reply: ') + item.adminReply;
        row.appendChild(reply);
      }
      list.appendChild(row);
    });
  }

  function renderMarkers() {
    $$('[data-review-marker]').forEach((node) => node.remove());
    const screen = currentScreen();
    const legacyScreen = screen === 'home' ? 'home' : 'community';
    state.reviews.filter((item) => (
      item.screen === screen || item.screen.startsWith(legacyScreen + ' ·')
    )).forEach((item, index) => {
      const marker = document.createElement('button');
      marker.type = 'button';
      marker.className = 'review-marker';
      marker.dataset.reviewUi = 'true';
      marker.dataset.reviewMarker = item.publicId;
      marker.dataset.status = item.status;
      marker.style.left = `${item.x * 100}vw`;
      marker.style.top = `${item.y * 100}vh`;
      marker.textContent = String(index + 1);
      marker.title = item.message + ' · ' + statusText(item.status);
      marker.onclick = () => {
        $('#review-list').hidden = false;
        renderList();
      };
      document.body.appendChild(marker);
    });
  }

  async function loadReviews() {
    try {
      const result = await api('/reviews');
      state.reviews = result.reviews || [];
      renderList();
      renderMarkers();
    } catch (error) {
      notify(error.message);
    }
  }

  function openForm(element) {
    const rect = element.getBoundingClientRect();
    const selectedScreen = currentScreen();
    const modal = document.createElement('div');
    modal.className = 'review-modal';
    modal.dataset.reviewUi = 'true';
    modal.innerHTML = `<form class="review-form">
      <h2>这里需要怎么改？</h2>
      <p data-selected-context></p>
      <div class="review-form-row">
        <label>意见类型<select name="category"><option value="layout">布局</option><option value="copy">文案</option><option value="feature">交互</option><option value="product">产品逻辑</option></select></label>
        <label>优先级<select name="priority"><option value="normal">一般</option><option value="high">重要</option><option value="low">不急</option></select></label>
      </div>
      <label>具体意见<textarea name="message" required minlength="2" maxlength="1000" placeholder="直接说你的感觉，例如：我不知道这里点了会发生什么"></textarea></label>
      <div class="review-form-actions"><button class="secondary" type="button" data-review-cancel>取消</button><button type="submit">提交</button></div>
    </form>`;
    window.chatcommonsI18n.translateSubtree(modal);
    $('[data-selected-context]', modal).textContent = l('你选择了：', 'You selected: ') + (visibleText(element) || l('页面上的这个位置', 'this part of the page'));
    document.body.appendChild(modal);
    const form = $('.review-form', modal);
    $('[data-review-cancel]', modal).onclick = () => modal.remove();
    modal.onclick = (event) => { if (event.target === modal) modal.remove(); };
    form.onsubmit = async (event) => {
      event.preventDefault();
      const submit = $('button[type="submit"]', form);
      submit.disabled = true;
      try {
        let screenshot = '';
        if (window.html2canvas) {
          try {
            const canvas = await window.html2canvas(document.documentElement, {
              x: scrollX, y: scrollY, width: innerWidth, height: innerHeight,
              windowWidth: innerWidth, windowHeight: innerHeight,
              scale: 0.65, useCORS: true, logging: false,
              ignoreElements: (node) => node.hasAttribute?.('data-review-ui'),
            });
            screenshot = canvas.toDataURL('image/jpeg', 0.68);
            if (screenshot.length > 1450000) screenshot = canvas.toDataURL('image/jpeg', 0.45);
          } catch (_) { screenshot = ''; }
        }
        const payload = {
          surface: 'prototype', screen: selectedScreen,
          targetId: selectorFor(element), targetText: visibleText(element),
          x: Math.min(1, Math.max(0, (rect.left + rect.width / 2) / innerWidth)),
          y: Math.min(1, Math.max(0, (rect.top + rect.height / 2) / innerHeight)),
          viewportWidth: innerWidth, viewportHeight: innerHeight,
          category: form.elements.category.value, priority: form.elements.priority.value,
          message: form.elements.message.value.trim(), screenshot,
        };
        await api('/reviews', { method: 'POST', body: JSON.stringify(payload) });
        modal.remove();
        await loadReviews();
        notify(l('意见已提交', 'Feedback submitted'));
      } catch (error) {
        notify(error.message);
        submit.disabled = false;
      }
    };
    form.elements.message.focus();
  }

  const toolbar = document.createElement('aside');
  toolbar.className = 'review-toolbar';
  toolbar.dataset.reviewUi = 'true';
  toolbar.innerHTML = `<strong>原型评审</strong><small>正常操作页面；需要评论时再点“标注意见”。</small><div class="review-toolbar-actions"><button type="button" data-review-select>标注意见</button><button type="button" class="secondary" data-review-list>已有意见</button></div><div class="review-list" id="review-list" hidden></div>`;
  window.chatcommonsI18n.translateSubtree(toolbar);
  document.body.appendChild(toolbar);
  $('[data-review-select]').onclick = () => setSelecting(!state.selecting);
  $('[data-review-list]').onclick = () => { const list = $('#review-list'); list.hidden = !list.hidden; if (!list.hidden) renderList(); };
  document.addEventListener('mouseover', (event) => {
    if (!state.selecting || event.target.closest('[data-review-ui]')) return;
    clearHighlight();
    state.highlighted = event.target;
    event.target.classList.add('review-highlight');
  }, true);
  document.addEventListener('click', (event) => {
    if (!state.selecting || event.target.closest('[data-review-ui]')) return;
    event.preventDefault();
    event.stopPropagation();
    const element = event.target;
    setSelecting(false);
    openForm(element);
  }, true);
  document.addEventListener('keydown', (event) => { if (event.key === 'Escape' && state.selecting) setSelecting(false); });
  window.addEventListener('resize', renderMarkers);
  window.addEventListener('chatcommons:screen-change', renderMarkers);
  window.addEventListener('chatcommons:locale-change', () => {
    setSelecting(state.selecting);
    renderList();
    renderMarkers();
  });
  new MutationObserver(renderMarkers).observe($('#app-shell'), { subtree: true, attributes: true, attributeFilter: ['hidden'] });
  loadReviews();
}());
