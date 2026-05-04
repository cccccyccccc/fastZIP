import { invoke } from "@tauri-apps/api/core";
import { useTaskStore } from "../state/taskStore";
import { useTauriEvent } from "../hooks/useTauriEvent";
import type { TaskProgressEvent, TaskCompletedEvent, TaskFailedEvent, TaskCanceledEvent } from "../types";

export default function TasksPage() {
  const tasks = useTaskStore((s) => s.tasks);
  const updateProgress = useTaskStore((s) => s.updateProgress);
  const completeTask = useTaskStore((s) => s.completeTask);
  const failTask = useTaskStore((s) => s.failTask);
  const cancelTaskStore = useTaskStore((s) => s.cancelTask);
  const clearTasks = useTaskStore((s) => s.clearTasks);

  useTauriEvent<TaskProgressEvent>("task-progress", (p) => {
    updateProgress(p.task_id, p.bytes_processed, p.speed_mbps, p.elapsed);
  });
  useTauriEvent<TaskCompletedEvent>("task-completed", (p) => completeTask(p.task_id));
  useTauriEvent<TaskFailedEvent>("task-failed", (p) => failTask(p.task_id, p.error));
  useTauriEvent<TaskCanceledEvent>("task-canceled", (p) => cancelTaskStore(p.task_id));

  const cancel = (taskId: number) => {
    invoke("cancel_archive_task", { taskId });
    cancelTaskStore(taskId);
  };

  const activeCount = tasks.filter((t) => t.status === "running").length;

  return (
    <div className="flex flex-col h-full gap-4">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-bold">
          Tasks {activeCount > 0 && <span className="text-sm text-blue-600">({activeCount} active)</span>}
        </h2>
        {tasks.length > 0 && (
          <button onClick={clearTasks} className="px-3 py-1.5 text-xs text-gray-500 hover:text-red-600 rounded-lg border border-gray-200 dark:border-gray-700">
            Clear all
          </button>
        )}
      </div>

      {tasks.length === 0 && (
        <p className="text-gray-400 text-sm">No tasks yet. Start an extraction or compression to see progress here.</p>
      )}

      <div className="flex-1 min-h-0 overflow-auto space-y-2">
        {tasks.map((t) => (
          <div key={t.id} className="border border-gray-200 dark:border-gray-700 rounded-lg p-4">
            <div className="flex items-center justify-between mb-2">
              <div>
                <span className="text-sm font-medium">{t.label}</span>
                <span className={`ml-2 text-xs px-2 py-0.5 rounded-full ${
                  t.status === "running" ? "bg-blue-100 text-blue-700" :
                  t.status === "completed" ? "bg-green-100 text-green-700" :
                  t.status === "failed" ? "bg-red-100 text-red-700" :
                  "bg-gray-100 text-gray-500"
                }`}>
                  {t.status}
                </span>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-xs text-gray-500">
                  {t.speedMbps.toFixed(1)} MB/s &middot; {formatDuration(t.elapsed)}
                </span>
                {t.status === "running" && (
                  <button onClick={() => cancel(t.id)} className="px-2 py-1 text-xs text-red-600 hover:bg-red-50 rounded">
                    Cancel
                  </button>
                )}
              </div>
            </div>
            {t.status === "running" && (
              <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-1.5">
                <div
                  className="bg-blue-600 h-1.5 rounded-full transition-all duration-150"
                  style={{ width: `${Math.min(100, t.totalBytes > 0 ? (t.bytesProcessed / t.totalBytes) * 100 : 50)}%` }}
                />
              </div>
            )}
            <div className="mt-1 text-xs text-gray-500">{formatBytes(t.bytesProcessed)}</div>
            {t.error && <p className="mt-1 text-xs text-red-600">{t.error}</p>}
          </div>
        ))}
      </div>
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}

function formatDuration(secs: number): string {
  if (secs < 60) return `${secs.toFixed(0)}s`;
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}m ${s}s`;
}
