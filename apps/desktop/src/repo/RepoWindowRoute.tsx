import { useMemo } from "react";
import { useTheme } from "../theme/useTheme";
import { RepoWindow } from "./RepoWindow";
import { ipcRepoApi } from "./repoApi";

/** `/repo/<id>`: the window for an opened repository. */
export function RepoWindowRoute({ id }: { id: number }) {
  useTheme();
  const api = useMemo(() => ipcRepoApi(id), [id]);
  return <RepoWindow api={api} aiRepo={id} />;
}
