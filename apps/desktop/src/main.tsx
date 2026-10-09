import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./theme/fonts";
import "./theme/tokens.css";

async function root() {
  // Dev/test-only route: the merge editor against fixtures, with the IPC mocked.
  if (
    import.meta.env.VITE_IPC_MOCK === "1" &&
    window.location.pathname.startsWith("/dev/merge")
  ) {
    document.documentElement.dataset.theme =
      new URLSearchParams(window.location.search).get("theme") ?? "dark";
    document.body.style.margin = "0";
    const { DevMerge } = await import("./merge-editor/DevMerge");
    return <DevMerge />;
  }
  const request = window.location.pathname.match(/^\/(merge|repo)\/(\d+)$/);
  if (request) {
    const id = Number(request[2]);
    if (request[1] === "merge") {
      const { MergeRequestView } = await import("./requests/MergeRequestView");
      return <MergeRequestView id={id} />;
    }
    const { RepoRequestView } = await import("./requests/RepoRequestView");
    return <RepoRequestView id={id} />;
  }
  return <App />;
}

void root().then((element) => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>{element}</React.StrictMode>,
  );
});
