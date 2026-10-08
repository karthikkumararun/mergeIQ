import { useState } from "react";
import "./theme/fonts";
import "./theme/tokens.css";
import { Home } from "./views/Home";
import { Settings } from "./views/Settings";

function App() {
  const [view, setView] = useState<"home" | "settings">("home");

  if (view === "settings") {
    return <Settings onClose={() => setView("home")} />;
  }
  return <Home onOpenSettings={() => setView("settings")} />;
}

export default App;
