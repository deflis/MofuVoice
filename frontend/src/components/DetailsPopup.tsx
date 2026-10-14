import { Notifications } from "./Notifications";
import { createPortal } from "react-dom";
import { useSettingsLocked } from "../settingsHooks";
import { useId, useRef, type ReactNode } from "react";

export function DetailsPopup({
  title,
  children,
  onOpen,
}: {
  title: string;
  children: ReactNode;
  onOpen?: () => void;
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
        詳細設定
      </button>
      {createPortal(
        <dialog ref={dialog} className="details-popup" aria-labelledby={titleId}>
          <header>
            <h2 id={titleId}>{title}の詳細設定</h2>
          </header>
          <Notifications />
          <p className="muted">調整が終わったら閉じて、設定を保存してください。</p>
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
