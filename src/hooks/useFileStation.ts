import { useCallback, useEffect, useRef, useState } from "react";
import { ATOLL_REACTION_MS } from "../atollTransitions";
import { eatReactionForCount } from "../fileStationTiers";
import type { AtollReaction } from "../AtollLogo";
import { manageAsyncUnlisten } from "../asyncUnlisten";
import { getDemoStagedFiles, isFileStationDemoMode } from "../demoSnapshot";
import { isTauriRuntime } from "../tauri/runtime";
import {
  getStagedFiles,
  onFileStationChanged,
  onIslandDragDropEvent,
  removeStagedFile,
  clearStagedFiles,
  stageFiles,
  type StageFilesResult,
  type StagedFile,
} from "../tauri";

export interface FileStationApi {
  stagedFiles: StagedFile[];
  stagedCount: number;
  /** True while files are being dragged over the island (mouth-open state). */
  dragOverIsland: boolean;
  /** Last drop outcome for toast copy (evicted/skipped); null once consumed. */
  lastStageResult: StageFilesResult | null;
  stashReaction: AtollReaction | null;
  stashReactionKey: number;
  /** Play a one-shot stash reaction (spit on panel open, eat on drop). */
  playStashReaction: (reaction: AtollReaction) => void;
  removeStaged: (id: string) => void;
  clearStaged: () => void;
}

/**
 * File staging station: owns the staged list, listens for native drag-drop
 * (enter/over → mouth open, drop → stage + eat reaction by new total) and
 * plays one-shot stash reactions with a replay key, mirroring how
 * useAtollReaction drives state-transition reactions.
 */
export function useFileStation(): FileStationApi {
  const [stagedFiles, setStagedFiles] = useState<StagedFile[]>([]);
  const [dragOverIsland, setDragOverIsland] = useState(false);
  const [lastStageResult, setLastStageResult] = useState<StageFilesResult | null>(null);
  const [stashReaction, setStashReaction] = useState<AtollReaction | null>(null);
  const [stashReactionKey, setStashReactionKey] = useState(0);
  const reactionTimerRef = useRef(0);

  const playStashReaction = useCallback((reaction: AtollReaction) => {
    setStashReaction(reaction);
    setStashReactionKey((key) => key + 1);
    window.clearTimeout(reactionTimerRef.current);
    reactionTimerRef.current = window.setTimeout(() => {
      setStashReaction(null);
    }, ATOLL_REACTION_MS[reaction]);
  }, []);

  useEffect(() => () => window.clearTimeout(reactionTimerRef.current), []);

  useEffect(() => {
    // Browser demo mode (`?demo=fileStation`): seed rows locally so the panel
    // and its animations can be inspected without the Tauri backend.
    if (isFileStationDemoMode()) {
      setStagedFiles(getDemoStagedFiles());
      return;
    }
    getStagedFiles()
      .then(setStagedFiles)
      .catch(() => undefined);
    const unsubscribeChanged = manageAsyncUnlisten(
      onFileStationChanged((files) => {
        setStagedFiles(files);
      }),
    );
    return () => {
      unsubscribeChanged();
    };
  }, []);

  useEffect(() => {
    const unsubscribeDrop = manageAsyncUnlisten(
      onIslandDragDropEvent((kind, paths) => {
        if (kind === "enter" || kind === "over") {
          setDragOverIsland(true);
          return;
        }
        setDragOverIsland(false);
        if (kind !== "drop") {
          return;
        }
        void stageFiles(paths)
          .then((result) => {
            if (!result || result.added === 0) {
              return;
            }
            setStagedFiles(result.files);
            setLastStageResult(result);
            const reaction = eatReactionForCount(result.files.length);
            if (reaction) {
              playStashReaction(reaction);
            }
          })
          .catch(() => undefined);
      }),
    );
    return () => {
      unsubscribeDrop();
    };
  }, [playStashReaction]);

  const removeStaged = useCallback((id: string) => {
    if (!isTauriRuntime()) {
      setStagedFiles((files) => files.filter((file) => file.id !== id));
      return;
    }
    removeStagedFile(id)
      .then(setStagedFiles)
      .catch(() => undefined);
  }, []);

  const clearStaged = useCallback(() => {
    if (!isTauriRuntime()) {
      setStagedFiles([]);
      return;
    }
    clearStagedFiles()
      .then(setStagedFiles)
      .catch(() => undefined);
  }, []);

  return {
    stagedFiles,
    stagedCount: stagedFiles.length,
    dragOverIsland,
    lastStageResult,
    stashReaction,
    stashReactionKey,
    playStashReaction,
    removeStaged,
    clearStaged,
  };
}
