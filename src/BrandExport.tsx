import { AtollLogo } from "./AtollLogo";
import { AgentMascot, AGENT_ACCENT } from "./AgentMascot";
import type { ClawdMood } from "./ClawdMascot";
import {
  APP_LOGO_STATE_LABELS,
  APP_STATE_ACTIVITY_MAP,
  type AppLogoState,
  APP_LOGO_STATES,
} from "./logoStates";

const MASCOT_MOODS: { mood: ClawdMood; label: string }[] = [
  { mood: "calm", label: "calm" },
  { mood: "alert", label: "alert" },
  { mood: "happy", label: "happy" },
  { mood: "worried", label: "worried" },
  { mood: "sad", label: "sad" },
  { mood: "sleeping", label: "sleeping" },
  { mood: "dead", label: "dead" },
];

const AGENTS = [
  { id: "claude", label: "Claude" },
  { id: "codex", label: "Codex" },
  { id: "cursor", label: "Cursor" },
  { id: "gemini", label: "Gemini" },
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
                  accent={AGENT_ACCENT[agent.id]?.accent}
                  accentDark={AGENT_ACCENT[agent.id]?.accentDark}
                />
              </div>
              <figcaption style={{ fontSize: 12, opacity: 0.8 }}>
                {agent.label}
              </figcaption>
            </figure>
          ))}
        </div>
      </section>

      <section style={{ marginTop: 40 }}>
        <h2 style={{ fontSize: 14, letterSpacing: "0.08em", opacity: 0.65 }}>
          CURSOR CUBE — ALL MOODS
        </h2>
        <div
          style={{
            display: "flex",
            gap: 20,
            alignItems: "flex-end",
            flexWrap: "wrap",
          }}
        >
          {MASCOT_MOODS.map(({ mood, label }) => (
            <figure key={mood} style={{ margin: 0, textAlign: "center" }}>
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
                  agent="cursor"
                  mood={mood}
                  size={72}
                  accent={AGENT_ACCENT.cursor.accent}
                  accentDark={AGENT_ACCENT.cursor.accentDark}
                />
              </div>
              <figcaption style={{ fontSize: 11, opacity: 0.75 }}>{label}</figcaption>
            </figure>
          ))}
        </div>
      </section>

      <section style={{ marginTop: 40 }}>
        <h2 style={{ fontSize: 14, letterSpacing: "0.08em", opacity: 0.65 }}>
          AGENT FAMILY — calm
        </h2>
        <div
          style={{
            display: "flex",
            gap: 40,
            alignItems: "flex-end",
            flexWrap: "wrap",
          }}
        >
          {(["claude", "codex", "cursor"] as const).map((agent) => (
            <figure key={agent} style={{ margin: 0, textAlign: "center" }}>
              <div
                style={{
                  width: 120,
                  height: 120,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  margin: "0 auto 8px",
                  background: "rgba(255,255,255,0.04)",
                  borderRadius: 12,
                }}
              >
                <AgentMascot
                  agent={agent}
                  mood="calm"
                  size={96}
                  accent={AGENT_ACCENT[agent]?.accent}
                  accentDark={AGENT_ACCENT[agent]?.accentDark}
                />
              </div>
              <figcaption style={{ fontSize: 12, opacity: 0.8 }}>
                {agent === "claude" ? "Clawd" : agent === "codex" ? "Codex" : "Cursor Cube"}
              </figcaption>
            </figure>
          ))}
        </div>
      </section>
    </main>
  );
}
