(function () {
  const VIEWBOX = "-20 -34 152 136";
  const LEGS = [16, 30.4, 72, 86.4];
  const EYE = "#1a1a1a";

  const AGENTS = {
    claude: { type: "clawd", mood: "calm" },
    codex: {
      type: "codex",
      mood: "calm",
      accent: "#61d8f7",
      accentDark: "#3d9fb8",
      glowInner: "rgba(158, 220, 255, 0.75)",
      glowOuter: "rgba(97, 216, 247, 0.28)",
    },
    gemini: {
      type: "clawd",
      mood: "calm",
      accent: "#b2e578",
      accentDark: "#7aa44d",
    },
  };

  function mixHex(from, to, amount) {
    const parse = (hex) => {
      const value = Number.parseInt(hex.slice(1), 16);
      return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
    };
    const [r1, g1, b1] = parse(from);
    const [r2, g2, b2] = parse(to);
    const mix = (a, b) => Math.round(a + (b - a) * amount);
    const channel = (v) => v.toString(16).padStart(2, "0");
    return `#${channel(mix(r1, r2))}${channel(mix(g1, g2))}${channel(mix(b1, b2))}`;
  }

  function clawdPalette(accent, accentDark) {
    return {
      body: accent || "#c27c5c",
      bodyTop: accent ? mixHex(accent, "#ffffff", 0.12) : "#d08a68",
      dark: accentDark || (accent ? mixHex(accent, "#000000", 0.35) : "#8b5a42"),
    };
  }

  function codexPalette(accent, accentDark) {
    const dark = accentDark || mixHex(accent, "#000000", 0.35);
    return {
      body: accent,
      bodyTop: mixHex(accent, "#ffffff", 0.18),
      dark,
      bezel: mixHex(dark, "#000000", 0.22),
      bezelRim: mixHex(accent, "#ffffff", 0.12),
      outline: mixHex(accent, "#ffffff", 0.42),
      prompt: mixHex(accent, "#ffffff", 0.55),
    };
  }

  function renderClawd(palette, mood) {
    const legs = LEGS.map(
      (x, i) =>
        `<rect class="clawd-leg clawd-leg-${i}" x="${x}" y="56" width="9.6" height="20" fill="${palette.dark}"/>`,
    ).join("");

    const blush =
      mood === "alert"
        ? `<ellipse cx="22" cy="42" rx="6" ry="3.2" fill="#ffb4b4" opacity="0.65"/>
           <ellipse cx="90" cy="42" rx="6" ry="3.2" fill="#ffb4b4" opacity="0.65"/>`
        : "";

    const bang =
      mood === "alert"
        ? `<g class="clawd-bang">
             <rect x="53" y="-30" width="6" height="14" rx="1.5" fill="#f8dda0"/>
             <rect x="53" y="-13" width="6" height="5" rx="1.5" fill="#f8dda0"/>
           </g>`
        : "";

    return `<span class="clawd is-${mood}" aria-hidden="true">
      <svg class="clawd-svg" viewBox="${VIEWBOX}" preserveAspectRatio="xMidYMid meet" shape-rendering="crispEdges">
        <ellipse cx="68" cy="92" rx="32" ry="4" fill="rgba(0,0,0,0.18)"/>
        <g class="clawd-body">
          <rect class="clawd-claw clawd-claw-left" x="-4" y="25.6" width="12" height="14.4" fill="${palette.body}"/>
          <rect class="clawd-claw clawd-claw-right" x="104" y="25.6" width="12" height="14.4" fill="${palette.body}"/>
          <rect x="8" y="0" width="96" height="56" fill="${palette.body}"/>
          <rect x="8" y="0" width="96" height="9" fill="${palette.bodyTop}"/>
          <rect class="clawd-eye" x="28" y="12" width="8" height="16" fill="${EYE}"/>
          <rect class="clawd-eye" x="76" y="12" width="8" height="16" fill="${EYE}"/>
          ${blush}
          ${legs}
        </g>
        ${bang}
      </svg>
    </span>`;
  }

  function renderCodex(palette, mood) {
    return `<span class="codex is-${mood}" aria-hidden="true" style="--codex-glow-inner:${palette.glowInner};--codex-glow-outer:${palette.glowOuter}">
      <svg class="codex-svg" viewBox="${VIEWBOX}" preserveAspectRatio="xMidYMid meet">
        <ellipse cx="68" cy="92" rx="34" ry="5" fill="rgba(0,0,0,0.32)"/>
        <g class="codex-body" shape-rendering="crispEdges">
          <rect x="38" y="-5" width="32" height="6" fill="${palette.body}" stroke="${palette.outline}" stroke-width="2.5"/>
          <rect x="16" y="0" width="88" height="56" fill="${palette.body}" stroke="${palette.outline}" stroke-width="2.5"/>
          <rect x="16" y="0" width="88" height="6" fill="${palette.bodyTop}"/>
          <rect class="codex-screen" x="18" y="5" width="84" height="49" fill="${palette.bezel}" stroke="${palette.bezelRim}" stroke-width="2.5"/>
          <rect x="20" y="7" width="80" height="45" fill="#0c1018"/>
          <text x="60" y="30" text-anchor="middle" dominant-baseline="central" fill="${palette.prompt}" font-family="ui-monospace,monospace" font-size="38" font-weight="700">
            <tspan>&gt;</tspan><tspan class="codex-cursor">_</tspan>
          </text>
          <rect class="codex-leg codex-leg-0" x="38" y="56" width="9.6" height="20" fill="${palette.dark}" stroke="${palette.outline}" stroke-width="2.5"/>
          <rect class="codex-leg codex-leg-1" x="72" y="56" width="9.6" height="20" fill="${palette.dark}" stroke="${palette.outline}" stroke-width="2.5"/>
        </g>
      </svg>
    </span>`;
  }

  function renderAgent(agentId, moodOverride) {
    const config = AGENTS[agentId];
    if (!config) return "";
    const mood = moodOverride || config.mood;

    if (config.type === "codex") {
      const palette = {
        ...codexPalette(config.accent, config.accentDark),
        glowInner: config.glowInner,
        glowOuter: config.glowOuter,
      };
      return renderCodex(palette, mood);
    }

    return renderClawd(clawdPalette(config.accent, config.accentDark), mood);
  }

  function init() {
    document.querySelectorAll(".mascot-slot[data-agent], .agent-mascot-slot[data-agent]").forEach((slot) => {
      const agentId = slot.dataset.agent;
      slot.innerHTML = renderAgent(agentId, slot.dataset.mood || undefined);

      const card = slot.closest(".agent-mascot-card");
      if (card) {
        card.dataset.agent = agentId;
      }
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
