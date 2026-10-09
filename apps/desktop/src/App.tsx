import { useEffect, useState } from "react";
import "./theme/fonts";
import "./theme/tokens.css";
import { commands, events } from "./ipc/bindings";
import { isIpcMock } from "./ipc/mock";
import { Home } from "./views/Home";
import { Settings, type SettingsSection } from "./views/Settings";

const SECTIONS: SettingsSection[] = ["general", "cli", "lockfiles", "ai"];

function asSection(name: string | null): SettingsSection | null {
  return SECTIONS.find((s) => s === name) ?? null;
}

function App() {
  const [view, setView] = useState<
    { name: "home" } | { name: "settings"; section: SettingsSection }
  >({ name: "home" });
  // Bumped after command-line setup changes so Home's status cards reload.
  const [cliVersion, setCliVersion] = useState(0);

  // Other windows (the merge editor's "Set up AI") ask for a settings section: either by
  // event, or, if this window did not exist yet, through what they left for it to find.
  useEffect(() => {
    if (isIpcMock) return;
    let stop: (() => void) | undefined;
    let cancelled = false;
    void commands.takePendingSettings().then((name) => {
      const section = asSection(name);
      if (section && !cancelled) setView({ name: "settings", section });
    });
    void events.openSettings
      .listen((e) => {
        const section = asSection(e.payload.section);
        if (section) setView({ name: "settings", section });
      })
      .then((off) => {
        if (cancelled) off();
        else stop = off;
      });
    return () => {
      cancelled = true;
      stop?.();
    };
  }, []);

  if (view.name === "settings") {
    return (
      <Settings
        section={view.section}
        onClose={() => setView({ name: "home" })}
        onCliChanged={() => setCliVersion((v) => v + 1)}
      />
    );
  }
  return (
    <Home
      cliVersion={cliVersion}
      onOpenSettings={(section = "general") =>
        setView({ name: "settings", section })
      }
    />
  );
}

export default App;
