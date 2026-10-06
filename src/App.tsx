import { useEffect } from "react";

import { PageView } from "./components/PageView";
import { Sidebar } from "./components/Sidebar";
import { useOwnWindowKeys } from "./shortcut/useOwnWindowKeys";
import { useShell } from "./store/shell";

export function App() {
  const page = useShell((s) => s.page);
  const loadAppInfo = useShell((s) => s.loadAppInfo);
  useOwnWindowKeys();

  useEffect(() => {
    void loadAppInfo();
  }, [loadAppInfo]);

  return (
    <div className="flex h-full">
      <Sidebar />
      <main className="min-w-0 flex-1 overflow-y-auto">
        <PageView page={page} />
      </main>
    </div>
  );
}
