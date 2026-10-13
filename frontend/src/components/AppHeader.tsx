import { useHeaderStatus } from "../audioHooks";

export function AppHeader() {
  const status = useHeaderStatus();
  return (
    <header>
      <h1>Voice Changer</h1>
      <output className="status-badge">{status}</output>
    </header>
  );
}
