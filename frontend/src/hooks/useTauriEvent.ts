import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export function useTauriEvent<T>(event: string, handler: (payload: T) => void) {
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    const start = async () => {
      unlisten = await listen<T>(event, (e) => handler(e.payload));
    };
    start();
    return () => {
      if (unlisten) unlisten();
    };
  }, [event, handler]);
}
