import type { ReactNode } from "react";
import TitleBar from "./TitleBar";
import SideNav from "./SideNav";
import Toast from "./Toast";

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <div className="h-screen flex flex-col bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 overflow-hidden">
      <TitleBar />
      <div className="flex flex-1 min-h-0">
        <SideNav />
        <main className="flex-1 min-w-0 overflow-auto p-4">{children}</main>
      </div>
      <Toast />
    </div>
  );
}
