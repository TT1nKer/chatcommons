function hasReviewAccess(): boolean {
  const incoming = new URLSearchParams(window.location.search).get('review');
  if (incoming && incoming.length >= 40) return true;
  try {
    return (sessionStorage.getItem('chatcommons-review-token')?.length ?? 0) >= 40;
  } catch {
    return false;
  }
}

function appendStylesheet(href: string): void {
  const link = document.createElement('link');
  link.rel = 'stylesheet';
  link.href = href;
  link.dataset.clientReviewAsset = 'true';
  document.head.appendChild(link);
}

function appendScript(src: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = src;
    script.dataset.clientReviewAsset = 'true';
    script.onload = () => resolve();
    script.onerror = () => reject(new Error(`Could not load review asset: ${src}`));
    document.head.appendChild(script);
  });
}

export async function mountReviewOverlay(): Promise<void> {
  if (!hasReviewAccess() || window.location.protocol === 'tauri:') return;
  const parent = new URL('../', window.location.href);
  appendStylesheet(new URL('review.css?v=20260723.11', parent).href);
  await appendScript(new URL('i18n.js?v=20260723.11', parent).href);
  await appendScript(new URL('review.js?v=20260723.11', parent).href);
}
