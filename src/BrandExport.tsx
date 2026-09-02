import { useEffect, useState } from "react";
import {
  AtollLogo,
  ATOLL_MICRO_MOTIONS,
  type AtollActivity,
  type AtollMicroMotion,
  type AtollReaction,
} from "./AtollLogo";
import { AgentMascot, AGENT_ACCENT } from "./AgentMascot";
import type { ClawdMood } from "./ClawdMascot";
import {
  ACTIVITY_HINTS,
  ACTIVITY_LABELS,
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

/** 全部姿态（应用态 + 彩蛋），调试台 / 资产导出共用顺序。 */
export const ATOLL_ACTIVITIES: AtollActivity[] = [
  "idle",
  "coding",
  "reading",
  "thinking",
  "coffee",
  "idea",
  "slacking",
  "napping",
  "dead",
  "fishing",
  "stargazing",
  "garden",
  "music",
  "gaming",
];

export function getBrandExportMode(): boolean {
  if (typeof window === "undefined") return false;
  return new URLSearchParams(window.location.search).get("export") === "brand";
}

/** ?static=1：姿态定格（供截图导出）；默认实时播放（交互调试）。 */
function useStaticBrandMode(): boolean {
  return (
    typeof window !== "undefined" &&
    new URLSearchParams(window.location.search).get("static") === "1"
  );
}

function PlaygroundCard({
  children,
  title,
  hint,
  item,
}: {
  children: React.ReactNode;
  title: string;
  hint: string;
  item?: string;
}) {
  return (
    <figure
      data-export-item={item}
      style={{
        margin: 0,
        textAlign: "center",
        width: 128,
      }}
    >
      <div
        style={{
          width: 108,
          height: 120,
          display: "flex",
          alignItems: "flex-end",
          justifyContent: "center",
          margin: "0 auto 8px",
          background: "rgba(255,255,255,0.04)",
          borderRadius: 12,
        }}
      >
        {children}
      </div>
      <figcaption style={{ fontSize: 12, opacity: 0.85 }}>{title}</figcaption>
      <div style={{ fontSize: 10, opacity: 0.5, marginTop: 2 }}>{hint}</div>
    </figure>
  );
}

function ReactionDemo({
  label,
  initial,
  target,
  reaction,
}: {
  label: string;
  initial: AtollActivity;
  target: AtollActivity;
  reaction: AtollReaction;
}) {
  const [activity, setActivity] = useState<AtollActivity>(initial);
  const [shot, setShot] = useState<{ kind: AtollReaction; key: number } | null>(null);
  const replay = () => {
    setActivity(target);
    setShot((prev) => ({ kind: reaction, key: (prev?.key ?? 0) + 1 }));
  };
  return (
    <figure style={{ margin: 0, textAlign: "center", width: 128 }}>
      <div
        style={{
          width: 108,
          height: 120,
          display: "flex",
          alignItems: "flex-end",
          justifyContent: "center",
          margin: "0 auto 8px",
          background: "rgba(255,255,255,0.04)",
          borderRadius: 12,
        }}
      >
        <AtollLogo
          activity={activity}
          size={64}
          reaction={shot?.kind ?? null}
          reactionKey={shot?.key ?? 0}
          idleIntervalSec={999_999}
        />
      </div>
      <figcaption style={{ fontSize: 12, opacity: 0.85 }}>{label}</figcaption>
      <button
        onClick={replay}
        style={{
          marginTop: 6,
          fontSize: 11,
          padding: "3px 12px",
          borderRadius: 6,
          background: "#1f2937",
          color: "#dbe4ec",
          border: "1px solid #374151",
          cursor: "pointer",
        }}
      >
        重放
      </button>
    </figure>
  );
}

function MicroDemo({ name }: { name: AtollMicroMotion }) {
  const [active, setActive] = useState<AtollMicroMotion | null>(null);
  useEffect(() => {
    let t1 = 0;
    let t2 = 0;
    const cycle = () => {
      t1 = window.setTimeout(() => {
        setActive(name);
        t2 = window.setTimeout(() => {
          setActive(null);
          cycle();
        }, 1800);
      }, 3000);
    };
    cycle();
    return () => {
      window.clearTimeout(t1);
      window.clearTimeout(t2);
    };
  }, [name]);
  return (
    <PlaygroundCard title={ACTIVITY_LABELS.idle + " · " + name} hint={`micro · ${name}`}>
      <AtollLogo activity="idle" size={64} microMotion={active} idleIntervalSec={999_999} />
    </PlaygroundCard>
  );
}

function AtollPlayground() {
  const staticMode = useStaticBrandMode();
  const [speed, setSpeed] = useState(1);

  useEffect(() => {
    document.getAnimations().forEach((a) => {
      a.playbackRate = speed;
    });
  }, [speed]);

  const speedBtn = (value: number) => (
    <button
      key={value}
      onClick={() => setSpeed(value)}
      style={{
        fontSize: 12,
        padding: "4px 14px",
        borderRadius: 6,
        marginRight: 6,
        background: speed === value ? "#38BDD8" : "#1f2937",
        color: speed === value ? "#06222b" : "#dbe4ec",
        border: "1px solid #374151",
        cursor: "pointer",
      }}
    >
      {value}×
    </button>
  );

  return (
    <section style={{ marginTop: 40 }}>
      <h2 style={{ fontSize: 14, letterSpacing: "0.08em", opacity: 0.65 }}>
        ATOLL PLAYGROUND — 全姿态 / 微动作 / 状态反应
        {staticMode ? "（static 模式：姿态定格供截图）" : ""}
      </h2>
      {!staticMode && (
        <div style={{ marginBottom: 14 }}>
          <span style={{ fontSize: 12, opacity: 0.6, marginRight: 8 }}>播放速度</span>
          {[0.5, 1, 2].map(speedBtn)}
        </div>
      )}
      <div
        data-export="atoll-activities"
        style={{
          display: "flex",
          gap: 16,
          alignItems: "flex-end",
          flexWrap: "wrap",
        }}
      >
        {ATOLL_ACTIVITIES.map((act) => (
          <PlaygroundCard
            key={act}
            item={`atoll-act-${act}`}
            title={ACTIVITY_LABELS[act]}
            hint={ACTIVITY_HINTS[act]}
          >
            <AtollLogo activity={act} size={64} motionPaused={staticMode} />
          </PlaygroundCard>
        ))}
      </div>

      <h2
        style={{
          fontSize: 12,
          letterSpacing: "0.08em",
          opacity: 0.55,
          margin: "28px 0 10px",
        }}
      >
        IDLE MICRO MOTIONS
      </h2>
      <div style={{ display: "flex", gap: 16, alignItems: "flex-end", flexWrap: "wrap" }}>
        {ATOLL_MICRO_MOTIONS.map((name) => (
          <MicroDemo key={name} name={name} />
        ))}
      </div>

      <h2
        style={{
          fontSize: 12,
          letterSpacing: "0.08em",
          opacity: 0.55,
          margin: "28px 0 10px",
        }}
      >
        STATE REACTIONS（点击重放）
      </h2>
      <div style={{ display: "flex", gap: 16, alignItems: "flex-end", flexWrap: "wrap" }}>
        <ReactionDemo label="cheer · 审批清空/任务完成" initial="coding" target="idle" reaction="cheer" />
        <ReactionDemo label="collapse · 掉线挣扎瘫倒" initial="idle" target="dead" reaction="collapse" />
        <ReactionDemo label="revive · 恢复在线苏醒" initial="dead" target="idle" reaction="revive" />
      </div>
    </section>
  );
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

      <AtollPlayground />

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
                  animated={false}
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

      {(["cursor", "codex"] as const).map((agent) => (
        <section key={agent} style={{ marginTop: 40 }}>
          <h2 style={{ fontSize: 14, letterSpacing: "0.08em", opacity: 0.65 }}>
            {agent === "cursor" ? "CURSOR" : "CODEX"} — ALL MOODS
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
                    agent={agent}
                    mood={mood}
                    size={72}
                    animated={false}
                    accent={AGENT_ACCENT[agent].accent}
                    accentDark={AGENT_ACCENT[agent].accentDark}
                  />
                </div>
                <figcaption style={{ fontSize: 11, opacity: 0.75 }}>{label}</figcaption>
              </figure>
            ))}
          </div>
        </section>
      ))}

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
                  animated={false}
                  accent={AGENT_ACCENT[agent]?.accent}
                  accentDark={AGENT_ACCENT[agent]?.accentDark}
                />
              </div>
              <figcaption style={{ fontSize: 12, opacity: 0.8 }}>
                {agent === "claude" ? "Clawd" : agent === "codex" ? "Codex" : "Cursor"}
              </figcaption>
            </figure>
          ))}
        </div>
      </section>
    </main>
  );
}
