export type UnlistenFn = () => void;

export function manageAsyncUnlisten(
  registration: Promise<UnlistenFn>,
): UnlistenFn {
  let disposed = false;
  let unlisten: UnlistenFn | null = null;

  registration
    .then((cleanup) => {
      if (disposed) {
        cleanup();
        return;
      }
      unlisten = cleanup;
    })
    .catch(() => undefined);

  return () => {
    disposed = true;
    unlisten?.();
    unlisten = null;
  };
}
