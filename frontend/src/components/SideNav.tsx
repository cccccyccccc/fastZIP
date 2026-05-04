import { useUIStore } from "../state/uiStore";

type PageKey = "extract" | "compress" | "tasks" | "fileManager" | "benchmark" | "settings" | "logs";

interface NavItem {
  key: PageKey;
  label: string;
  icon: string;
}

const items: NavItem[] = [
  { key: "extract", label: "Extract", icon: "⇩" },
  { key: "compress", label: "Compress", icon: "⇧" },
  { key: "tasks", label: "Tasks", icon: "≡" },
  { key: "fileManager", label: "Files", icon: "📁" },
  { key: "benchmark", label: "Benchmark", icon: "⚡" },
  { key: "logs", label: "Logs", icon: "📝" },
  { key: "settings", label: "Settings", icon: "⚙" },
];

export default function SideNav() {
  const activePage = useUIStore((s) => s.activePage);
  const setPage = useUIStore((s) => s.setPage);

  return (
    <nav className="w-16 bg-gray-50 dark:bg-gray-850 border-r border-gray-200 dark:border-gray-700 flex flex-col items-center py-2 gap-1 shrink-0">
      {items.map((item) => (
        <button
          key={item.key}
          onClick={() => setPage(item.key)}
          title={item.label}
          className={`w-12 h-12 rounded-lg flex flex-col items-center justify-center text-xs transition-colors ${
            activePage === item.key
              ? "bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300"
              : "text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700"
          }`}
        >
          <span className="text-lg leading-none">{item.icon}</span>
          <span className="text-[10px] leading-tight mt-0.5">{item.label}</span>
        </button>
      ))}
    </nav>
  );
}
