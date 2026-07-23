(function () {
  'use strict';
  const params = new URLSearchParams(location.search);
  const incoming = params.get('owner');
  if (incoming) {
    sessionStorage.setItem('chatcommons-owner-token', incoming);
    params.delete('owner');
    history.replaceState({}, document.title, `${location.pathname}${params.toString() ? `?${params}` : ''}${location.hash}`);
  }
  const token = sessionStorage.getItem('chatcommons-owner-token') || '';
  const inbox = document.querySelector('#inbox');
  const errorBox = document.querySelector('#admin-error');
  const labels = { layout:'布局',copy:'文案',feature:'交互',product:'产品逻辑',low:'不急',normal:'一般',high:'重要' };
  const thankYouReply = 'Thank you for taking the time to explain this. Your feedback genuinely helps ChatCommons improve. We have made the corresponding change and moved it to “Ready for review.” Please keep pointing out anything that still feels unclear or unnatural.\n\n感谢你认真说明这个问题。你的反馈确实帮助 ChatCommons 变得更好；我们已经完成对应修改并将它移到“待验收”。如果还有不清楚或不自然的地方，请继续指出。';

  async function api(path, options = {}) {
    const response = await fetch(`./api/admin${path}`, { credentials:'same-origin', ...options, headers:{ 'Content-Type':'application/json','X-Owner-Token':token,...(options.headers||{}) } });
    const body = await response.json().catch(() => ({}));
    if (!response.ok) throw new Error(body.error || '读取失败');
    return body;
  }

  async function loadScreenshot(id, image) {
    const response = await fetch(`./api/admin/reviews/${id}/image`, { headers: { 'X-Owner-Token': token }, credentials: 'same-origin' });
    if (!response.ok) throw new Error('截图读取失败');
    const blob = await response.blob();
    image.src = URL.createObjectURL(blob);
  }

  function node(tag, className, text) { const element=document.createElement(tag); if(className) element.className=className; if(text!==undefined) element.textContent=text; return element; }
  function render(items) {
    inbox.replaceChildren();
    document.querySelector('#pending-count').textContent = String(items.filter((item) => ['pending','in_progress'].includes(item.status)).length);
    document.querySelector('#review-count').textContent = String(items.filter((item) => item.status === 'client_review').length);
    document.querySelector('#total-count').textContent = String(items.length);
    if (!items.length) { inbox.appendChild(node('p','empty','还没有收到意见。')); return; }
    items.forEach((item) => {
      const card=node('article','review-card');
      const shot=node('div','review-shot');
      if(item.hasScreenshot){ const image=document.createElement('img'); image.alt=`意见 ${item.publicId} 的页面截图`; image.loading='lazy'; image.onclick=()=>window.open(image.src,'_blank','noopener'); shot.appendChild(image); loadScreenshot(item.id,image).catch(()=>{shot.textContent='截图读取失败';}); } else shot.textContent='没有截图，仍可按文字处理';
      const meta=node('div','review-meta'); meta.append(node('small','',`${item.publicId} · ${new Date(item.createdAt).toLocaleString('zh-CN')}`),node('h2','',item.targetText||item.screen),node('p','',item.message));
      const tags=node('div','tags'); [labels[item.category]||item.category,labels[item.priority]||item.priority,item.screen].forEach((value)=>tags.appendChild(node('span','',value))); meta.appendChild(tags);
      const controls=node('form','review-controls');
      const statusLabel=node('label','','处理状态'); const select=document.createElement('select'); select.name='status'; [['pending','待确认'],['in_progress','处理中'],['client_review','待朋友验收'],['completed','已完成'],['rejected','暂不处理'],['withdrawn','朋友已撤回']].forEach(([value,text])=>{const option=document.createElement('option');option.value=value;option.textContent=text;option.selected=item.status===value;select.appendChild(option);}); statusLabel.appendChild(select);
      const replyLabel=node('label','','给朋友的回复'); const textarea=document.createElement('textarea'); textarea.name='reply'; textarea.maxLength=1000; textarea.placeholder='说明已修改什么，或者为什么暂不处理'; textarea.value=item.adminReply||''; replyLabel.appendChild(textarea);
      const thank=node('button','thank-button','感谢并待验收'); thank.type='button';
      const save=node('button','', '保存处理结果'); save.type='submit'; controls.append(statusLabel,replyLabel,thank,save);
      const persist=async(button)=>{thank.disabled=true;save.disabled=true;try{await api(`/reviews/${item.id}`,{method:'PATCH',body:JSON.stringify({status:select.value,adminReply:textarea.value.trim()})});button.textContent='已发送';await load();}catch(error){showError(error.message);thank.disabled=false;save.disabled=false;}};
      thank.onclick=()=>{select.value='client_review';textarea.value=thankYouReply;persist(thank);};
      controls.onsubmit=(event)=>{event.preventDefault();persist(save);};
      card.append(shot,meta,controls); inbox.appendChild(card);
    });
  }

  function showError(message){errorBox.textContent=message;errorBox.hidden=false;}
  async function load(){errorBox.hidden=true;if(token.length<40){showError('管理员链接无效或未提供。');inbox.replaceChildren();return;}try{const result=await api('/reviews');render(result.reviews||[]);}catch(error){showError(error.message);inbox.replaceChildren();}}
  document.querySelector('#refresh').onclick=load;
  load();
}());
