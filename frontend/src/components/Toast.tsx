import { useUIStore } from "../state/uiStore";

const kindStyles: Record<string, string> = {
  info: "bg-blue-600",
  error: "bg-red-600",
  success: "bg-green-600",
};

export default function Toast() {
  const toasts = useUIStore((s) => s.toasts);
  const removeToast = useUIStore((s) => s.removeToast);

  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={`${kindStyles[t.kind]} text-white px-4 py-2 rounded-lg shadow-lg cursor-pointer text-sm`}
          onClick={() => removeToast(t.id)}
        >
          {t.message}
        </div>
      ))}
    </div>
  );
}
