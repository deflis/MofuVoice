import { useLogStore } from "./store";

export function Notifications() {
  const error = useLogStore((state) => state.error);
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
