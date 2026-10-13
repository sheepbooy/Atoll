import { useEffect, useState } from "react";
import { AgentMascot, type MascotAction } from "./AgentMascot";
import { MascotEnvironmentProvider } from "./mascotMotion";
import { useReducedMotion } from "./animationTiming";
import type { AgentKind } from "./tauri";
import type { ClawdMood } from "./mascotShared";

const CHARACTERS: { agent: AgentKind; label: string; parts: string; gesture: string }[] = [
  { agent: "claude", label: "Claude", parts: "眼睛 · 双爪 · 四条腿", gesture: "看一看、挠脸、交替轻敲与挥爪" },
  { agent: "codex", label: "Codex", parts: "花瓣 · > 提示符 · _ 光标", gesture: "左右敲键、举瓣提问、张瓣雀跃与摇头" },
  { agent: "cursor", label: "Cursor", parts: "三面立方体 · 内部箭头", gesture: "箭头先探出，立方体各面随后响应" },
  { agent: "gemini", label: "Gemini", parts: "四个星尖 · 连续轮廓", gesture: "左右忙活、歪头招手、蓄力雀跃与摇头收身" },
  { agent: "opencode", label: "OpenCode", parts: "像素外框 · 内部白块", gesture: "探头点头、左右查看、顶起盒子与缩头摇头" },
  { agent: "zcode", label: "ZCode", parts: "底板 · Z 字 · 三段笔画", gesture: "前倾用力、抬身等待、挺胸弹起与后仰回撤" },
];
const STATES: { label: string; mood: ClawdMood; activity: "idle" | "working" | "rest" }[] = [
  { label: "空闲", mood: "calm", activity: "idle" },
  { label: "工作", mood: "calm", activity: "working" },
  { label: "待审批", mood: "alert", activity: "idle" },
  { label: "担忧", mood: "worried", activity: "rest" },
  { label: "开心", mood: "happy", activity: "idle" },
  { label: "低落", mood: "sad", activity: "rest" },
  { label: "休眠", mood: "sleeping", activity: "rest" },
  { label: "离线", mood: "dead", activity: "rest" },
];
const ACTIONS: [MascotAction, string][] = [["started", "开始工作"], ["attention", "提醒"], ["approved", "通过"], ["denied", "拒绝"], ["turnEnded", "回合结束"], ["join", "加入"], ["leave", "离开"]];
const STATE_ACTIONS: (MascotAction | null)[] = [null,"started","attention","concerned","approved","downcast","sleeping",null];
const COMPARISONS: { label: string; mood: ClawdMood; activity: "idle" | "working" | "rest"; action: MascotAction | null }[] = [
  { label:"工作", mood:"calm", activity:"working", action:null },
  { label:"待审批", mood:"alert", activity:"rest", action:"attention" },
  { label:"通过", mood:"calm", activity:"rest", action:"approved" },
  { label:"拒绝", mood:"calm", activity:"rest", action:"denied" },
];

export function getAgentPreviewMode() {
  return typeof window !== "undefined" && new URLSearchParams(window.location.search).get("preview") === "agents";
}

export function AgentMascotPreviewPage() {
  const [state, setState] = useState(1);
  const [selected, setSelected] = useState(() => {
    const agent = new URLSearchParams(window.location.search).get("agent");
    return CHARACTERS.some(c => c.agent === agent) ? agent! : "all";
  });
  const [comparison, setComparison] = useState(() => new URLSearchParams(window.location.search).get("view") === "states");
  const [beat, setBeat] = useState(0);
  const [replays, setReplays] = useState<Record<string,number>>({});
  const [shot, setShot] = useState<{ kind: MascotAction; key: number } | null>(null);
  const [rate, setRate] = useState(1);
  const [paused, setPaused] = useState(false);
  const [saving, setSaving] = useState(false);
  const [monochrome, setMonochrome] = useState(false);
  const [labels, setLabels] = useState(true);
  const [extras, setExtras] = useState(false);
  const reduced = useReducedMotion();
  const pose = STATES[state];
  const fire = (kind: MascotAction) => setShot(prev => ({ kind, key: (prev?.key ?? 0) + 1 }));
  useEffect(() => {
    if (!comparison || paused || saving || reduced) return;
    const timer = window.setInterval(() => setBeat(n => n + 1),4000/rate);
    return () => window.clearInterval(timer);
  }, [comparison,paused,saving,reduced,rate,beat,replays]);
  return <MascotEnvironmentProvider ambientPaused={saving} feedback={null} rate={rate}>
    <main className={`agent-playground${monochrome ? " is-monochrome" : ""}${extras ? "" : " hides-extras"}`}>
      <header className="agent-playground-heading">
        <p>ATOLL / CHARACTER MOTION</p>
        <h1>让形象自己表演</h1>
        <div>探头、招手、雀跃与回撤；在实际 18px 下核对状态辨识度。</div>
      </header>
      <section className="agent-playground-controls" aria-label="动画控制">
        <button aria-pressed={comparison} onClick={() => { setComparison(c => !c); setShot(null); }}>{comparison ? "查看单一状态" : "状态并排对照"}</button>
        {!comparison && <label>状态 <select aria-label="状态" value={state} onChange={e => { setState(Number(e.target.value)); setShot(null); }}>
          {STATES.map((s, i) => <option value={i} key={s.label}>{s.label}</option>)}
        </select></label>}
        <label>播放对象 <select aria-label="播放对象" value={selected} onChange={e => setSelected(e.target.value)}>
          <option value="all">全部形象</option>{CHARACTERS.map(c => <option value={c.agent} key={c.agent}>{c.label}</option>)}
        </select></label>
        <label>速度 <select aria-label="速度" value={rate} onChange={e => { setRate(Number(e.target.value)); setShot(null); }}>
          {[.25, .5, 1, 2].map(n => <option value={n} key={n}>{n}×</option>)}
        </select></label>
        <button onClick={() => setPaused(p => !p)}>{paused ? "继续" : "暂停并复位"}</button>
        <label><input type="checkbox" checked={saving} onChange={e => setSaving(e.target.checked)} />省电</label>
        <label><input type="checkbox" checked={monochrome} onChange={e => setMonochrome(e.target.checked)} />统一颜色</label>
        <label><input type="checkbox" checked={labels} onChange={e => setLabels(e.target.checked)} />形象名称</label>
        <label><input type="checkbox" checked={extras} onChange={e => setExtras(e.target.checked)} />外围装饰</label>
        {comparison ? <button onClick={() => setBeat(n => n+1)} disabled={paused || reduced}>重放所有反馈</button> : <div className="agent-playground-actions">{ACTIONS.map(([kind, label]) => <button key={kind} onClick={() => fire(kind)} disabled={paused || reduced}>{label}</button>)}</div>}
        {reduced && <div role="status">系统已开启减弱动态效果，当前展示静态姿态。</div>}
      </section>
      {comparison ? <section className="agent-comparison" aria-label="同一形象的四种状态对照">
        {CHARACTERS.filter(c => selected === "all" || selected === c.agent).map(c => <section className="agent-comparison-row" data-agent={c.agent} key={c.agent}>
          <div className="agent-comparison-heading">
            {labels && <h2>{c.label}<small>{c.gesture}</small></h2>}
            <button onClick={() => setReplays(prev => ({...prev,[c.agent]:(prev[c.agent] ?? 0)+1}))} disabled={paused || reduced}>重放反馈</button>
          </div>
          {COMPARISONS.map(cell => {
            const common = { agent:c.agent,mood:cell.mood,activity:cell.activity,action:cell.action,
              actionKey:`${beat}:${replays[c.agent] ?? 0}`,instanceKey:`compare-${c.agent}-${cell.label}`,motionPaused:paused,
              ...(monochrome ? { accent:"#e6e9ed",accentDark:"#949aa4" } : {}) };
            return <article className="agent-comparison-cell" data-compare-state={cell.action ?? "working"} key={cell.label}>
              <h3>{cell.label}</h3>
              <div className="agent-comparison-stage"><AgentMascot {...common} size={84} /></div>
              <div className="agent-playground-sizes">{[18,14].map(size => <div key={size}>
                <span className="agent-playground-slot"><AgentMascot {...common} size={size} /></span><small>{size}px</small>
              </div>)}</div>
            </article>;
          })}
        </section>)}
      </section> : <section className="agent-playground-grid" aria-label="形象动作对照">
        {CHARACTERS.map(c => {
          const active = selected === "all" || selected === c.agent;
          const common = { agent: c.agent, mood: pose.mood, activity: pose.activity, instanceKey: `preview-${c.agent}`,
            motionPaused: paused, action: active ? shot?.kind : null, actionKey: shot?.key,
            ...(monochrome ? { accent: "#e6e9ed", accentDark: "#949aa4" } : {}) };
          return <article className="agent-playground-card" data-agent={c.agent} key={c.agent}>
            {labels && <h2>{c.label}<small>{c.parts}</small></h2>}
            <div className="agent-playground-stage"><AgentMascot {...common} size={112} /></div>
            {labels && <p>{c.gesture}</p>}
            <div className="agent-playground-sizes">{[14, 18, 28, 36].map(size => <div key={size}>
              <span className="agent-playground-slot"><AgentMascot {...common} size={size} /></span><small>{size}px</small>
            </div>)}</div>
            <button className="agent-playground-replay" onClick={() => { setSelected(c.agent); fire(shot?.kind ?? STATE_ACTIONS[state]!); }} disabled={paused || reduced || (!shot && !STATE_ACTIONS[state])}>单独重放</button>
          </article>;
        })}
      </section>}
      <footer>{comparison ? "工作持续循环 · 反馈每 4 秒重播 · 18px / 14px 按实际显示尺寸对照" : "空闲约 12 秒一轮 · 工作含停顿 · 提醒约 4 秒一轮 · 一次性反馈完成后自动复位"}</footer>
    </main>
  </MascotEnvironmentProvider>;
}
