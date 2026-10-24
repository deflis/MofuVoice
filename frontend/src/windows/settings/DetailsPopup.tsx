import { Notifications } from "./Notifications";
import { createPortal } from "react-dom";
import { useSettingsLocked } from "./hooks";
import { useId, useRef, type ReactNode } from "react";

export function DetailsPopup({
  title,
  children,
  onOpen,
  buttonLabel,
}: {
  title: string;
  children: ReactNode;
  onOpen?: () => void;
  buttonLabel: string;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const locked = useSettingsLocked();
  return (
    <>
      <button
        type="button"
        aria-haspopup="dialog"
        onClick={() => {
          onOpen?.();
          dialog.current?.showModal();
        }}
      >
        {buttonLabel}
      </button>
      {createPortal(
        <dialog ref={dialog} className="details-popup" aria-labelledby={titleId}>
          <header>
            <h2 id={titleId}>
              {title}の{buttonLabel}
            </h2>
          </header>
          <Notifications />
          <fieldset className="settings details-popup-content" disabled={locked}>
            {children}
          </fieldset>
          <footer>
            <button type="button" onClick={() => dialog.current?.close()}>
              閉じる
            </button>
          </footer>
        </dialog>,
        document.body,
      )}
    </>
  );
}
