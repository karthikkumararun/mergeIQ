import type { EditorState, TransactionSpec } from "@codemirror/state";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useMemo, useState } from "react";
import { describe, expect, it } from "vitest";
import type { StructuralResolve } from "../../../ipc/bindings";
import { mockMergeDocument } from "../../../ipc/mock";
import { structuralFixtures } from "../../__fixtures__/structural";
import { counter } from "../../model/actions";
import { createResultState } from "../../model/state";
import { createStructuralParts } from ".";
import type { StructuralStore } from "./store";

const NAME = "structural-ts";
const fx = structuralFixtures[NAME];

function Harness({
  resolve,
  onStore,
}: {
  resolve: () => Promise<StructuralResolve>;
  onStore?: (s: StructuralStore) => void;
}) {
  const doc = useMemo(() => mockMergeDocument(NAME), []);
  const parts = useMemo(() => {
    const p = createStructuralParts(resolve);
    onStore?.(p.store);
    return p;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const [state, setState] = useState<EditorState>(() =>
    createResultState(fx.analysis),
  );
  const ctx = {
    analysis: fx.analysis,
    doc,
    state,
    view: null,
    dispatch: (spec: TransactionSpec) => setState((s) => s.update(spec).state),
  };
  return (
    <div>
      {parts.extension.toolbarItems?.(ctx)}
      <div style={{ position: "relative" }}>
        {parts.extension.resultOverlay?.(ctx)}
      </div>
      <pre data-testid="doc">{state.doc.toString()}</pre>
      <span data-testid="left">{counter(state).conflicts}</span>
    </div>
  );
}

const ready = () => Promise.resolve(fx.resolve);

describe("Editor integration", () => {
  it("Toolbar bulk structural", async () => {
    const user = userEvent.setup();
    render(<Harness resolve={ready} />);
    const button = await screen.findByRole("button", {
      name: /Resolve structurally \(3\)/,
    });
    expect(screen.getByTestId("structural-status")).toHaveTextContent(
      "Proposals ready · computed in 12 ms",
    );
    await user.click(button);
    expect(screen.getByTestId("doc")).toHaveTextContent("verbose: boolean;");
    expect(screen.getByTestId("doc")).toHaveTextContent("retries: number;");
    expect(screen.getByTestId("left")).toHaveTextContent("1");
    // Nothing structural is left to offer.
    expect(
      screen.queryByRole("button", { name: /Resolve structurally/ }),
    ).toBeNull();
  });

  it("Proposals are computed in the background", async () => {
    let finish!: (r: StructuralResolve) => void;
    render(
      <Harness
        resolve={() => new Promise<StructuralResolve>((r) => (finish = r))}
      />,
    );
    // The editor is usable at once; a status line says what is going on.
    expect(await screen.findByText("Finding structural merges…")).toBeVisible();
    expect(
      screen.queryByRole("button", { name: /Resolve structurally/ }),
    ).toBeNull();
    await act(async () => finish(fx.resolve));
    expect(
      await screen.findByRole("button", { name: /Resolve structurally/ }),
    ).toBeVisible();
  });

  it("Timeout", async () => {
    render(
      <Harness
        resolve={() =>
          Promise.resolve({ outcome: "TimedOut", elapsed_ms: 2000 })
        }
      />,
    );
    await waitFor(() =>
      expect(screen.getByTestId("structural-status")).toHaveTextContent(""),
    );
    expect(
      screen.queryByRole("button", { name: /Resolve structurally/ }),
    ).toBeNull();
  });

  it("Unsupported extension", async () => {
    render(
      <Harness
        resolve={() =>
          Promise.resolve({ outcome: "Unsupported", elapsed_ms: 0 })
        }
      />,
    );
    await waitFor(() =>
      expect(screen.getByTestId("structural-status")).toHaveTextContent(""),
    );
    expect(
      screen.queryByRole("button", { name: /Resolve structurally/ }),
    ).toBeNull();
  });
});

describe("Preview", () => {
  async function opened(index: number) {
    let store!: StructuralStore;
    const user = userEvent.setup();
    render(<Harness resolve={ready} onStore={(s) => (store = s)} />);
    await screen.findByRole("button", { name: /Resolve structurally/ });
    act(() => store.getState().open(index));
    return { user, store, dialog: await screen.findByRole("dialog") };
  }

  it("shows the explanation, the change and what was validated", async () => {
    const { dialog } = await opened(1);
    expect(
      within(dialog).getByRole("heading", {
        name: /Structural proposal · Options/,
      }),
    ).toBeVisible();
    expect(dialog).toHaveTextContent(
      "Left added verbose and right added retries.",
    );
    // The diff shows the new lines.
    expect(dialog).toHaveTextContent("verbose: boolean;");
    expect(dialog).toHaveTextContent("retries: number;");
    expect(dialog).toHaveTextContent("Covers 1 conflict");
    expect(dialog).toHaveTextContent("Validated: parses without errors");
    expect(dialog).toHaveFocus();
  });

  it("File-level proposals are titled with the file name", async () => {
    const { dialog } = await opened(0);
    expect(
      within(dialog).getByRole("heading", {
        name: /Structural proposal · options\.ts/,
      }),
    ).toBeVisible();
  });

  it("Apply proposal resolves just that proposal and closes the preview", async () => {
    const { user } = await opened(1);
    await user.click(screen.getByRole("button", { name: "Apply proposal" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByTestId("doc")).toHaveTextContent("retries: number;");
    expect(screen.getByTestId("doc")).not.toHaveTextContent("Auto,");
    expect(
      screen.getByRole("button", { name: /Resolve structurally \(2\)/ }),
    ).toBeVisible();
  });

  it("Dismiss removes the proposal from the offer", async () => {
    const { user } = await opened(1);
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByTestId("doc")).not.toHaveTextContent("verbose");
    expect(
      screen.getByRole("button", { name: /Resolve structurally \(2\)/ }),
    ).toBeVisible();
  });

  it("Escape closes the preview without changing anything", async () => {
    const { user } = await opened(2);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(
      screen.getByRole("button", { name: /Resolve structurally \(3\)/ }),
    ).toBeVisible();
  });
});
