import type { IslandSnapshot, PermissionRequest } from "./types";

// Offline/demo fallback state shared by the snapshot reader and the approval
// mutations: when Atoll's backend is unreachable, snapshots are served from
// this in-memory mirror so the island keeps rendering.
export let localRequests: PermissionRequest[] = [];
export let snapshotInFlight: Promise<IslandSnapshot> | null = null;

export function setLocalRequests(next: PermissionRequest[]) {
  localRequests = next;
}

export function setSnapshotInFlight(promise: Promise<IslandSnapshot> | null) {
  snapshotInFlight = promise;
}
