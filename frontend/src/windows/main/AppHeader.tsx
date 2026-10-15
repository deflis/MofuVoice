import { useMainStore } from "./store";
import { headerLabel } from "./selectors";

export function AppHeader() {
  const status = useMainStore((state) => headerLabel(state.machine));
  return (
    <header>
      <h1>Voice Changer</h1>
      <output className="status-badge">{status}</output>
    </header>
  );
}
