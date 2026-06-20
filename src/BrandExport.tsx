import { AtollLogo } from "./AtollLogo";
import { AgentMascot } from "./AgentMascot";
import {
  APP_LOGO_STATE_LABELS,
  APP_STATE_ACTIVITY_MAP,
  type AppLogoState,
  APP_LOGO_STATES,
} from "./logoStates";

const AGENTS = [
  { id: "claude", label: "Claude", accent: undefined, accentDark: undefined },
  { id: "codex", label: "Codex", accent: "#61d8f7", accentDark: "#3d9fb8" },
  { id: "gemini", label: "Gemini", accent: "#b2e578", accentDark: "#7aa44d" },
] as const;

export function getBrandExportMode(): boolean {
  if (typeof window === "undefined") return false;
  return new URLSearchParams(window.location.search).get("export") === "brand";
}

export function BrandExportPage() {
  return (
    <main
      className="brand-export"
      style={{
        margin: 0,
        padding: 32,
        background: "#0a0b0d",
        color: "#e8eaed",
        fontFamily: "system-ui, sans-serif",
      }}
    >
      <section style={{ marginBottom: 40 }}>
        <h2 style={{ fontSize: 14, letterSpacing: "0.08em", opacity: 0.65 }}>
          ATOLL LOGO STATES
        </h2>
        <div
          data-export="atoll-states"
          style={{
            display: "flex",
            gap: 24,
            alignItems: "flex-end",
            flexWrap: "wrap",
          }}
        >
          {APP_LOGO_STATES.map((state) => (
            <figure
              key={state}
              data-export-item={`atoll-${state}`}
              style={{ margin: 0, textAlign: "center" }}
            >
              <div
                style={{
                  width: 96,
                  height: 108,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  margin: "0 auto 8px",
                }}
              >
                <AtollLogo
                  activity={APP_STATE_ACTIVITY_MAP[state as AppLogoState]}
                  size={72}
                  motionPaused
                />
              </div>
              <figcaption style={{ fontSize: 12, opacity: 0.8 }}>
                {APP_LOGO_STATE_LABELS[state as AppLogoState]}
              </figcaption>
            </figure>
          ))}
        </div>
      </section>

      <section>
        <h2 style={{ fontSize: 14, letterSpacing: "0.08em", opacity: 0.65 }}>
          AGENT MASCOTS
        </h2>
        <div
          data-export="agent-mascots"
          style={{
            display: "flex",
            gap: 32,
            alignItems: "flex-end",
            flexWrap: "wrap",
          }}
        >
          {AGENTS.map((agent) => (
            <figure
              key={agent.id}
              data-export-item={`agent-${agent.id}`}
              style={{ margin: 0, textAlign: "center" }}
            >
              <div
                style={{
                  width: 88,
                  height: 88,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  margin: "0 auto 8px",
                }}
              >
                <AgentMascot
                  agent={agent.id}
                  mood="calm"
                  size={64}
                  accent={agent.accent}
                  accentDark={agent.accentDark}
                />
              </div>
              <figcaption style={{ fontSize: 12, opacity: 0.8 }}>
                {agent.label}
              </figcaption>
            </figure>
          ))}
        </div>
      </section>
    </main>
  );
}
