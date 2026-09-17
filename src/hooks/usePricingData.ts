// Selected-agent tab state plus the pricing catalog that powers cost display.
// Owns the agent-tab derivation and the "new pending request forces its agent
// tab" sync effect. Extracted from App.tsx; behavior unchanged.
import { useEffect, useMemo, useRef, useState } from "react";
import type { IslandSnapshot, PermissionRequest } from "../tauri";
import { getPricing } from "../pricing";
import { pricingRateMap, type ModelPricingEntry } from "../pricing";
import { agentSortRank } from "../agents";
import type { AgentKind, PanelView } from "../appTypes";

interface UsePricingDataOptions {
  snapshot: IslandSnapshot;
  activeRequest: PermissionRequest | null;
  panelView: PanelView;
  navigationSeqRef: { current: number };
  setPanelView: (view: PanelView) => void;
}

export function usePricingData({
  snapshot,
  activeRequest,
  panelView,
  navigationSeqRef,
  setPanelView,
}: UsePricingDataOptions) {
  const [selectedAgent, setSelectedAgent] = useState<AgentKind | null>(null);
  const selectedAgentRef = useRef<AgentKind | null>(null);
  selectedAgentRef.current = selectedAgent;
  const [pricingModels, setPricingModels] = useState<ModelPricingEntry[]>([]);
  const lastSyncedRequestIdRef = useRef<string | null>(null);

  const pricingRates = pricingRateMap(pricingModels);

  const sessions = snapshot.sessions;
  const tabAgents = useMemo(() => {
    const seen = new Set<AgentKind>();
    sessions.forEach((session) => seen.add(session.agent));
    if (activeRequest) {
      seen.add(activeRequest.agent);
    }
    return Array.from(seen).sort(
      (a, b) => agentSortRank[a] - agentSortRank[b],
    );
  }, [sessions, activeRequest]);

  useEffect(() => {
    getPricing()
      .then((response) => setPricingModels(response.models))
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    if (tabAgents.length === 0) {
      setSelectedAgent(null);
      lastSyncedRequestIdRef.current = null;
      return;
    }

    // New pending request: force-select its agent and return to home so the
    // approval card is visible (not stuck on another agent's tab/subview).
    if (activeRequest?.id && activeRequest.id !== lastSyncedRequestIdRef.current) {
      lastSyncedRequestIdRef.current = activeRequest.id;
      setSelectedAgent(activeRequest.agent);
      if (panelView.kind !== "home") {
        ++navigationSeqRef.current;
        setPanelView({ kind: "home" });
      }
      return;
    }

    if (!activeRequest) {
      lastSyncedRequestIdRef.current = null;
    }

    if (selectedAgent && tabAgents.includes(selectedAgent)) {
      return;
    }
    setSelectedAgent(activeRequest?.agent ?? tabAgents[0]);
  }, [tabAgents, selectedAgent, activeRequest?.id, activeRequest?.agent, panelView.kind]);

  function handleSelectAgent(agent: AgentKind) {
    setSelectedAgent(agent);
    if (panelView.kind !== "home") {
      ++navigationSeqRef.current;
      setPanelView({ kind: "home" });
    }
  }

  return {
    selectedAgent,
    setSelectedAgent,
    selectedAgentRef,
    pricingModels,
    setPricingModels,
    pricingRates,
    tabAgents,
    handleSelectAgent,
  };
}
