import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { selectAdapter } from './adapter';
import { App } from './App';
import { mountReviewOverlay } from './review-overlay';
import './styles.css';

const root = document.querySelector('#root');
if (!root) {
  throw new Error('ChatCommons client root is missing.');
}

createRoot(root).render(
  <StrictMode>
    <App adapter={selectAdapter()} />
  </StrictMode>,
);

void mountReviewOverlay();
