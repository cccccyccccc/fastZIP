import { create } from "zustand";

type PageKey = "extract" | "compress" | "tasks" | "fileManager" | "benchmark" | "settings" | "logs";

interface UIState {
  activePage: PageKey;
  toasts: Toast[];
  setPage: (page: PageKey) => void;
  addToast: (message: string, kind?: "info" | "error" | "success") => void;
  removeToast: (id: number) => void;
}

export interface Toast {
  id: number;
  message: string;
  kind: "info" | "error" | "success";
}

let nextToastId = 1;

export const useUIStore = create<UIState>()((set) => ({
  activePage: "extract",
  toasts: [],

  setPage: (activePage) => set({ activePage }),

  addToast: (message, kind = "info") => {
    const id = nextToastId++;
    set((s) => ({ toasts: [...s.toasts, { id, message, kind }] }));
    setTimeout(() => {
      set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
    }, 4000);
  },

  removeToast: (id) =>
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));
