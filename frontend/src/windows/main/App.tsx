import { useMainStore } from "./store";
import { connectMainWindow } from "./listener";
import { Notifications } from "./Notifications";
import { Home } from "./Home";

export function MainApp() {
  const status = useMainStore((state) => state.status);

  let body;
  if (status === "bootstrapping") {
    body = (
      <div>
        <p>設定を読み込んでいます…</p>
      </div>
    );
  } else if (status === "bootstrapFailed") {
    body = (
      <div>
        <button type="button" onClick={() => connectMainWindow()}>
          設定の読み込みを再試行
        </button>
      </div>
    );
  } else if (status === "closing") {
    body = (
      <div>
        <p>終了しています…</p>
      </div>
    );
  } else {
    body = <Home />;
  }

  return (
    <main>
      <Notifications />
      {body}
    </main>
  );
}
