import { CursorMascot } from "./CursorMascot";
import { AGENT_ACCENT } from "./AgentMascot";
import type { ClawdMood } from "./ClawdMascot";

const MOODS: { mood: ClawdMood; label: string; hint: string }[] = [
  { mood: "calm", label: "calm", hint: "空闲 · 悬浮呼吸" },
  { mood: "alert", label: "alert", hint: "待审批 · 弹跳 + !!" },
  { mood: "happy", label: "happy", hint: "完成 · 星心特效" },
  { mood: "worried", label: "worried", hint: "异常 · 发绿 + 汗" },
  { mood: "sad", label: "sad", hint: "低落 · 垂眉下沉" },
  { mood: "sleeping", label: "sleeping", hint: "休眠 · zzz" },
  { mood: "dead", label: "dead", hint: "离线 · 灰化" },
];

export function getCursorPreviewMode(): boolean {
  if (typeof window === "undefined") return false;
  return new URLSearchParams(window.location.search).get("preview") === "cursor";
}

export function CursorMascotPreviewPage() {
  const accent = AGENT_ACCENT.cursor.accent;
  const accentDark = AGENT_ACCENT.cursor.accentDark;

  return (
    <main className="cursor-preview">
      <header className="cursor-preview-header">
        <p className="cursor-preview-eyebrow">Atoll Agent Mascot</p>
        <h1 className="cursor-preview-title">Cursor Cube</h1>
        <p className="cursor-preview-subtitle">
          等距立方体 · 左脸表情 · 品牌凹槽 · 七种 mood
        </p>
      </header>

      <section className="cursor-preview-hero" aria-label="Cursor calm">
        <div className="cursor-preview-hero-stage">
          <CursorMascot
            mood="calm"
            size={160}
            accent={accent}
            accentDark={accentDark}
          />
        </div>
        <p className="cursor-preview-hero-caption">calm — 默认态</p>
      </section>

      <section className="cursor-preview-grid" aria-label="Cursor moods">
        {MOODS.map(({ mood, label, hint }) => (
          <figure key={mood} className="cursor-preview-card">
            <div className="cursor-preview-card-stage">
              <CursorMascot
                mood={mood}
                size={112}
                accent={accent}
                accentDark={accentDark}
              />
            </div>
            <figcaption className="cursor-preview-card-label">{label}</figcaption>
            <p className="cursor-preview-card-hint">{hint}</p>
          </figure>
        ))}
      </section>

      <footer className="cursor-preview-footer">
        accent {accent} · accentDark {accentDark}
      </footer>
    </main>
  );
}
