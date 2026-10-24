import { useMainStore } from "./store";
import { headerLabel } from "./selectors";

export function AppHeader() {
  const status = useMainStore((state) => headerLabel(state));
  return (
    <header>
      <h1>もふボイス (MofuVoice)</h1>
      <output className="status-badge">{status}</output>
    </header>
  );
}
