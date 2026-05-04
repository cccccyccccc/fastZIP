import { useEffect, useState, useRef } from "react";

interface LogEntry {
  id: number;
  time: Date;
  message: string;
  level: "info" | "warn" | "error";
}

let logId = 0;
const logListeners: Set<() => void> = new Set();
const logBuffer: LogEntry[] = [];

export function pushLog(message: string, level: "info" | "warn" | "error" = "info") {
  logBuffer.push({ id: logId++, time: new Date(), message, level });
  if (logBuffer.length > 500) logBuffer.shift();
  logListeners.forEach((cb) => cb());
}

export default function LogsPage() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [autoScroll, setAutoScroll] = useState(true);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const listener = () => setLogs([...logBuffer]);
    logListeners.add(listener);
    setLogs([...logBuffer]);
    return () => { logListeners.delete(listener); };
  }, []);

  useEffect(() => {
    if (autoScroll && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [logs, autoScroll]);

  const clearLogs = () => {
    logBuffer.length = 0;
    setLogs([]);
  };

  const levelColor = (level: string) =>
    level === "error" ? "text-red-600" : level === "warn" ? "text-yellow-600" : "text-gray-500";

  return (
    <div className="flex flex-col h-full gap-4">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-bold">Logs</h2>
        <div className="flex gap-2">
          <label className="flex items-center gap-1 text-xs text-gray-500">
            <input type="checkbox" checked={autoScroll} onChange={(e) => setAutoScroll(e.target.checked)} />
            Auto-scroll
          </label>
          <button onClick={clearLogs} className="px-3 py-1.5 text-xs text-gray-500 hover:text-red-600 rounded-lg border border-gray-200 dark:border-gray-700">
            Clear
          </button>
        </div>
      </div>

      <div ref={containerRef} className="flex-1 min-h-0 overflow-auto bg-gray-50 dark:bg-gray-850 rounded-lg p-3 font-mono text-xs">
        {logs.length === 0 ? (
          <p className="text-gray-400">No log entries yet.</p>
        ) : (
          logs.map((entry) => (
            <div key={entry.id} className="flex gap-2 leading-relaxed">
              <span className="text-gray-400 shrink-0">
                {entry.time.toLocaleTimeString()}
              </span>
              <span className={`shrink-0 w-10 ${levelColor(entry.level)}`}>
                [{entry.level.toUpperCase()}]
              </span>
              <span className="text-gray-700 dark:text-gray-300 break-all">{entry.message}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
