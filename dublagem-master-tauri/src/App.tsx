import * as Tabs from "@radix-ui/react-tabs";
import { Settings, ShieldCheck, SlidersHorizontal } from "lucide-react";
import { useEffect } from "react";
import { DubbingPanel } from "./features/dubbing/DubbingPanel";
import { ProjectExplorer } from "./features/project-explorer/ProjectExplorer";
import { SettingsPanel } from "./features/settings/SettingsPanel";
import { ValidationPanel } from "./features/validation/ValidationPanel";
import { useWorkspaceStore } from "./stores/workspaceStore";
import styles from "./App.module.css";

function App() {
  const load = useWorkspaceStore((state) => state.load);
  const appendLog = useWorkspaceStore((state) => state.appendLog);

  useEffect(() => {
    void load().catch((error: unknown) => {
      appendLog(
        error instanceof Error ? error.message : "Falha ao carregar configuracao.",
        "error"
      );
    });
  }, [appendLog, load]);

  return (
    <main className={styles.shell}>
      <ProjectExplorer />
      <Tabs.Root className={styles.workspace} defaultValue="dubbing">
        <header className={styles.topbar}>
          <div>
            <h1>Dublagem Master</h1>
            <p>Pipeline local em Rust para transcrição, tradução, síntese e validação.</p>
          </div>
          <Tabs.List className={styles.tabs}>
            <Tabs.Trigger value="dubbing">
              <SlidersHorizontal size={16} />
              Dublagem
            </Tabs.Trigger>
            <Tabs.Trigger value="validation">
              <ShieldCheck size={16} />
              Validação
            </Tabs.Trigger>
            <Tabs.Trigger value="settings">
              <Settings size={16} />
              Ajustes
            </Tabs.Trigger>
          </Tabs.List>
        </header>

        <Tabs.Content className={styles.content} value="dubbing">
          <DubbingPanel />
        </Tabs.Content>
        <Tabs.Content className={styles.content} value="validation">
          <ValidationPanel />
        </Tabs.Content>
        <Tabs.Content className={styles.content} value="settings">
          <SettingsPanel />
        </Tabs.Content>
      </Tabs.Root>
    </main>
  );
}

export default App;
