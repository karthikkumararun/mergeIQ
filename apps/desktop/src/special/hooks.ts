import { useCallback, useEffect, useRef, useState } from "react";

export type Async<T> =
  | { state: "loading" }
  | { state: "error"; message: string }
  | { state: "ready"; value: T };

const messageOf = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** Loads once per `key` (a new key reloads); stale results are dropped. */
export function useAsync<T>(load: () => Promise<T>, key: unknown): Async<T> {
  const [result, setResult] = useState<{ key: unknown; value: Async<T> }>({
    key,
    value: { state: "loading" },
  });
  const loadRef = useRef(load);
  // Keep the latest loader; declared first so it is current when the loading effect runs.
  useEffect(() => {
    loadRef.current = load;
  });
  useEffect(() => {
    let live = true;
    void loadRef
      .current()
      .then(
        (value) => live && setResult({ key, value: { state: "ready", value } }),
      )
      .catch(
        (e: unknown) =>
          live &&
          setResult({ key, value: { state: "error", message: messageOf(e) } }),
      );
    return () => {
      live = false;
    };
  }, [key]);
  return result.key === key ? result.value : { state: "loading" };
}

/** Runs one user action at a time, tracking whether it is busy and why it failed. */
export function useAction() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mounted = useRef(true);
  useEffect(
    () => () => {
      mounted.current = false;
    },
    [],
  );
  const run = useCallback(async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      if (mounted.current) setError(messageOf(e));
    } finally {
      if (mounted.current) setBusy(false);
    }
  }, []);
  return { busy, error, run, clearError: () => setError(null) };
}
