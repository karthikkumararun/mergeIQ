import { useState } from "react";
import type { ConflictLoad, StageMeta } from "../ipc/bindings";
import { typeLabel } from "../repo/describe";
import type { RepoApi } from "../repo/repoApi";
import type { ResolutionMethod } from "../repo/repoStore";
import { formatBytes, leftName, rightName, shortOid } from "./format";
import { useAction, useAsync } from "./hooks";
import { PanelShell } from "./PanelShell";
import styles from "./Panels.module.css";
import shell from "./Special.module.css";

interface Props {
  load: ConflictLoad;
  api: RepoApi;
  onBack: () => void;
  onResolved: (method: ResolutionMethod) => Promise<void>;
}

interface CardProps {
  title: React.ReactNode;
  meta?: string;
  stage: number;
  info: StageMeta | undefined;
  isImage: boolean;
  api: RepoApi;
  path: string;
  label: string;
  footer: React.ReactNode;
}

/** A side card: checkerboard preview (images), dimensions, size and blob. */
function SideCard({
  title,
  meta,
  stage,
  info,
  isImage,
  api,
  path,
  label,
  footer,
}: CardProps) {
  const blob = useAsync(
    () =>
      isImage && info ? api.stageBlob(path, stage) : Promise.resolve(null),
    `${path}:${stage}:${info?.oid}`,
  );
  const [dims, setDims] = useState<string | null>(null);
  return (
    <section aria-label={label} className={shell.card}>
      <div className={shell.cardHead}>
        <span className={shell.cardTitle}>{title}</span>
        {meta && <span className={shell.cardMeta}>{meta}</span>}
      </div>
      {isImage && (
        <div className={styles.checker}>
          {!info ? (
            <span className={styles.placeholder}>Not present on this side</span>
          ) : blob.state === "loading" ? (
            <span className={styles.placeholder}>Loading preview…</span>
          ) : blob.state === "error" ? (
            <span className={styles.placeholder}>
              Preview unavailable: {blob.message}
            </span>
          ) : blob.value ? (
            // An <img> never runs scripts, which is how SVG stays sandboxed.
            <img
              alt={`${label} preview`}
              src={`data:${blob.value.mime};base64,${blob.value.base64}`}
              onLoad={(e) =>
                setDims(
                  `${e.currentTarget.naturalWidth} × ${e.currentTarget.naturalHeight} px`,
                )
              }
            />
          ) : null}
        </div>
      )}
      {!info ? (
        !isImage && (
          <p className={styles.cardNote} style={{ padding: 16 }}>
            Not present on this side.
          </p>
        )
      ) : (
        <dl className={shell.meta}>
          {isImage && (
            <>
              <dt>Dimensions</dt>
              <dd>{dims ?? "—"}</dd>
            </>
          )}
          <dt>Size</dt>
          <dd>{formatBytes(info.size)}</dd>
          <dt>Blob</dt>
          <dd>{shortOid(info.oid)}</dd>
        </dl>
      )}
      <div className={styles.cardFoot}>{footer}</div>
    </section>
  );
}

/** A binary conflict (image previews when the name says so): pick a side, bytes written exactly. */
export function BinaryPanel({ load, api, onBack, onResolved }: Props) {
  const { entry, labels } = load;
  const isImage = entry.class.class === "Binary" && entry.class.isImage;
  const details = useAsync(() => api.details(entry.path), entry.path);
  const { busy, error, run } = useAction();
  const stages = details.state === "ready" ? details.value.stages : [];
  const stage = (n: number) => stages.find((s) => s.stage === n);
  const left = leftName(labels);
  const right = rightName(labels);
  const kind = isImage ? "image" : "file";
  const chooseSide = (side: "Ours" | "Theirs") =>
    run(async () => {
      await api.useSide(entry.path, side);
      await onResolved(side === "Ours" ? "Accepted Left" : "Accepted Right");
    });
  const button = (side: "Ours" | "Theirs") => {
    const present = !!stage(side === "Ours" ? 2 : 3);
    return (
      <button
        type="button"
        className={`${shell.btn} ${shell.wide}`}
        disabled={busy}
        onClick={() => void chooseSide(side)}
      >
        {side === "Ours" ? "Use Left" : "Use Right"}
        {!present && " (delete the file)"}
      </button>
    );
  };
  const both = entry.conflictType === "BothAdded";
  return (
    <PanelShell
      label="Binary conflict"
      display={entry.display}
      badge={isImage ? "Binary · image" : "Binary"}
      tone="info"
      subtitle={typeLabel(entry.conflictType)}
      title={
        both
          ? `Both sides added this ${kind}`
          : `Both sides changed this ${kind}`
      }
      lead="Binary files can’t be merged line by line. Pick the version to keep; its bytes are written exactly and staged."
      maxWidth={1280}
      onBack={onBack}
    >
      {details.state === "error" && (
        <p role="alert" className={shell.alert}>
          {details.message}
        </p>
      )}
      {error && (
        <p role="alert" className={shell.alert}>
          {error}
        </p>
      )}
      <div className={shell.grid}>
        <SideCard
          title="Base"
          meta={stage(1) ? `merge-base ${shortOid(stage(1)?.oid)}` : undefined}
          stage={1}
          info={stage(1)}
          isImage={isImage}
          api={api}
          path={entry.path}
          label="Base"
          footer="Reference only"
        />
        <SideCard
          title={
            <>
              Left · <span className={shell.mono}>{left}</span>
            </>
          }
          meta={load.context.ours[0]?.shortSha}
          stage={2}
          info={stage(2)}
          isImage={isImage}
          api={api}
          path={entry.path}
          label={`Left, ${left}`}
          footer={button("Ours")}
        />
        <SideCard
          title={
            <>
              Right · <span className={shell.mono}>{right}</span>
            </>
          }
          meta={load.context.theirs[0]?.shortSha}
          stage={3}
          info={stage(3)}
          isImage={isImage}
          api={api}
          path={entry.path}
          label={`Right, ${right}`}
          footer={button("Theirs")}
        />
      </div>
      {isImage && (
        <p className={shell.note}>
          SVG previews render as sandboxed images, never inline markup.
        </p>
      )}
    </PanelShell>
  );
}
