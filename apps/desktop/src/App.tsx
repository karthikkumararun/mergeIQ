import { useState } from "react";
import "./theme/fonts";
import "./theme/tokens.css";
import { Home } from "./views/Home";
import { Settings, type SettingsSection } from "./views/Settings";

function App() {
  const [view, setView] = useState<
    { name: "home" } | { name: "settings"; section: SettingsSection }
  >({ name: "home" });
  // Bumped after command-line setup changes so Home's status cards reload.
  const [cliVersion, setCliVersion] = useState(0);

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
