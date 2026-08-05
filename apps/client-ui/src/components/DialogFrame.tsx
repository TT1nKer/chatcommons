import { type ReactNode, useId } from 'react';

interface DialogFrameProps {
  title: string;
  closeLabel: string;
  onClose: () => void;
  children: ReactNode;
}

export function DialogFrame({
  title,
  closeLabel,
  onClose,
  children,
}: DialogFrameProps) {
  const titleId = useId();

  return (
    <div className="client-dialog-backdrop" role="presentation" onMouseDown={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}>
      <section className="client-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <header>
          <h2 id={titleId}>{title}</h2>
          <button type="button" onClick={onClose} aria-label={closeLabel}>×</button>
        </header>
        {children}
      </section>
    </div>
  );
}
