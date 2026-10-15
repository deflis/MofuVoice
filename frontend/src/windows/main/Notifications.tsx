import { useMainStore } from "./store";

export function Notifications() {
  const error = useMainStore((state) => state.error);
  return (
    <>
      {error && (
        <div className="error" role="alert">
          {error}
        </div>
      )}
    </>
  );
}
