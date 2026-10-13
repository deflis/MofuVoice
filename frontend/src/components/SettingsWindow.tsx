import { useRef } from "react";
import { SettingsPanel } from "./SettingsPanel";
import { Notifications } from "./Notifications";
import { SaveSettingsButton } from "./SaveSettingsButton";

export function SettingsWindow() {
  const dialog = useRef<HTMLDialogElement>(null);
  return (
    <>
      <button type="button" onClick={() => dialog.current?.showModal()}>
        設定
      </button>
      <dialog ref={dialog} className="settings-window" aria-labelledby="settings-title">
        <header>
          <h1 id="settings-title">Settings</h1>
          <button type="button" onClick={() => dialog.current?.close()} aria-label="設定を閉じる">
            閉じる
          </button>
        </header>
        <Notifications />
        <SettingsPanel />
        <SaveSettingsButton />
      </dialog>
    </>
  );
}
