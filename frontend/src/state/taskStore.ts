import { create } from "zustand";

export interface TaskEntry {
  id: number;
  kind: "extract" | "compress";
  label: string;
  bytesProcessed: number;
  totalBytes: number;
  speedMbps: number;
  elapsed: number;
  status: "running" | "completed" | "failed" | "canceled";
  error?: string;
}

interface TaskState {
  tasks: TaskEntry[];
  nextId: number;
  addTask: (kind: "extract" | "compress", label: string) => number;
  updateProgress: (id: number, bytes: number, speed: number, elapsed: number) => void;
  completeTask: (id: number) => void;
  failTask: (id: number, error: string) => void;
  cancelTask: (id: number) => void;
  clearTasks: () => void;
}

export const useTaskStore = create<TaskState>()((set, get) => ({
  tasks: [],
  nextId: 1,
  addTask: (kind, label): number => {
    const id = get().nextId;
    set((s) => ({
      nextId: s.nextId + 1,
      tasks: [
        ...s.tasks,
        { id, kind, label, bytesProcessed: 0, totalBytes: 0, speedMbps: 0, elapsed: 0, status: "running" },
      ],
    }));
    return id;
  },
  updateProgress: (id, bytes, speed, elapsed) =>
    set((s) => ({
      tasks: s.tasks.map((t) =>
        t.id === id ? { ...t, bytesProcessed: bytes, speedMbps: speed, elapsed } : t,
      ),
    })),
  completeTask: (id) =>
    set((s) => ({
      tasks: s.tasks.map((t) => (t.id === id ? { ...t, status: "completed" } : t)),
    })),
  failTask: (id, error) =>
    set((s) => ({
      tasks: s.tasks.map((t) => (t.id === id ? { ...t, status: "failed", error } : t)),
    })),
  cancelTask: (id) =>
    set((s) => ({
      tasks: s.tasks.map((t) => (t.id === id ? { ...t, status: "canceled" } : t)),
    })),
  clearTasks: () => set({ tasks: [] }),
}));
