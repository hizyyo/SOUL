import { useEffect, useId, useRef, type ReactNode } from 'react';

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

interface ModalProps {
  title: string;
  children: ReactNode;
  onClose: () => void;
  closeOnBackdrop?: boolean;
  closeOnEscape?: boolean;
  ariaDescribedBy?: string;
  titleStyle?: React.CSSProperties;
}

export function Modal({
  title,
  children,
  onClose,
  closeOnBackdrop = true,
  closeOnEscape = true,
  ariaDescribedBy,
  titleStyle,
}: ModalProps) {
  const titleId = useId();
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const previouslyFocused =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const dialog = dialogRef.current;
    const focusables = getFocusableElements(dialog);
    (focusables[0] ?? dialog)?.focus();

    const onKeyDown = (event: KeyboardEvent) => {
      if (shouldCloseModalForKey(event.key, closeOnEscape)) {
        event.preventDefault();
        onClose();
        return;
      }
      if (event.key !== 'Tab' || !dialog) return;

      const current = getFocusableElements(dialog);
      if (current.length === 0) {
        event.preventDefault();
        dialog.focus();
        return;
      }
      const first = current[0];
      const last = current[current.length - 1];
      if (!first || !last) return;
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };

    document.addEventListener('keydown', onKeyDown, true);
    return () => {
      document.removeEventListener('keydown', onKeyDown, true);
      previouslyFocused?.focus();
    };
  }, [closeOnEscape, onClose]);

  return (
    <div
      className="modal-backdrop"
      onMouseDown={(event) => {
        if (closeOnBackdrop && event.target === event.currentTarget) onClose();
      }}
    >
      <div
        ref={dialogRef}
        className="modal-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={ariaDescribedBy}
        tabIndex={-1}
      >
        <h3 id={titleId} style={{ margin: '0 0 12px', ...titleStyle }}>
          {title}
        </h3>
        {children}
      </div>
    </div>
  );
}

export function shouldCloseModalForKey(key: string, closeOnEscape: boolean): boolean {
  return key === 'Escape' && closeOnEscape;
}

function getFocusableElements(root: HTMLElement | null): HTMLElement[] {
  if (!root) return [];
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) => element.getAttribute('aria-hidden') !== 'true',
  );
}
