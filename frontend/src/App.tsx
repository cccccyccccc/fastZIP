import { useTheme } from "./hooks/useTheme";
import { I18nProvider } from "./components/I18nProvider";
import Layout from "./components/Layout";
import UpdateDialog from "./components/UpdateDialog";
import { useUIStore } from "./state/uiStore";
import ExtractPage from "./pages/ExtractPage";
import CompressPage from "./pages/CompressPage";
import TasksPage from "./pages/TasksPage";
import FileManagerPage from "./pages/FileManagerPage";
import BenchmarkPage from "./pages/BenchmarkPage";
import SettingsPage from "./pages/SettingsPage";
import LogsPage from "./pages/LogsPage";

function App() {
  useTheme();
  const activePage = useUIStore((s) => s.activePage);

  const page = () => {
    switch (activePage) {
      case "extract": return <ExtractPage />;
      case "compress": return <CompressPage />;
      case "tasks": return <TasksPage />;
      case "fileManager": return <FileManagerPage />;
      case "benchmark": return <BenchmarkPage />;
      case "settings": return <SettingsPage />;
      case "logs": return <LogsPage />;
    }
  };

  return (
    <I18nProvider>
      <Layout>
        {page()}
        <UpdateDialog />
      </Layout>
    </I18nProvider>
  );
}

export default App;
