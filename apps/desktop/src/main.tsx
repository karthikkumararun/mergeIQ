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
  // Dev/test-only route: Settings › Command line against an in-memory backend.
  if (
    import.meta.env.VITE_IPC_MOCK === "1" &&
    window.location.pathname.startsWith("/dev/cli")
  ) {
    const params = new URLSearchParams(window.location.search);
    document.documentElement.dataset.theme = params.get("theme") ?? "dark";
    const [{ CommandLine }, { createMockCliApi }] = await Promise.all([
      import("./settings/CommandLine"),
      import("./settings/mockCliApi"),
    ]);
    const api = createMockCliApi({
      platform: params.get("platform") ?? "macos",
      onPath: params.get("onPath") !== "0",
      installed: params.get("installed") === "1",
      configured: params.get("configured") === "1",
    });
    return (
      <div
        style={{
          padding: 24,
          minHeight: "100vh",
          background: "var(--bg)",
          color: "var(--text)",
          fontFamily: "var(--font-ui)",
        }}
      >
        <CommandLine api={api} />
      </div>
    );
  }
  // Dev/test-only route: the open-repository home body against an in-memory backend.
  if (
    import.meta.env.VITE_IPC_MOCK === "1" &&
    window.location.pathname.startsWith("/dev/home")
  ) {
    const params = new URLSearchParams(window.location.search);
    document.documentElement.dataset.theme = params.get("theme") ?? "dark";
    const [{ OpenRepository }, { createMockHomeApi }] = await Promise.all([
      import("./repo/OpenRepository"),
      import("./repo/mockHomeApi"),
    ]);
    const empty = params.get("recents") === "0";
    const api = createMockHomeApi({
      recents: empty ? [] : undefined,
      repos: [
        "/code/shop-web",
        "/code/work/payments-service",
        "/code/new-repo",
      ],
      picked: params.get("picked"),
    });
    return (
      <div
        style={{
          padding: 24,
          minHeight: "100vh",
          background: "var(--bg)",
          color: "var(--text)",
          fontFamily: "var(--font-ui)",
        }}
      >
        <OpenRepository api={api} />
      </div>
    );
  }
  const merge = window.location.pathname.match(/^\/merge\/(\d+)$/);
  if (merge) {
    const { MergeRequestView } = await import("./requests/MergeRequestView");
    return <MergeRequestView id={Number(merge[1])} />;
  }
  return <App />;
}

void root().then((element) => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>{element}</React.StrictMode>,
  );
});
