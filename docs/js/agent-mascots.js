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
    },
    gemini: {
      type: "clawd",
      mood: "calm",
      accent: "#b2e578",
      accentDark: "#7aa44d",
    },
    cursor: {
      type: "cursor",
      mood: "calm",
      accent: "#a78bfa",
      accentDark: "#7c5fd4",
    },
  };

  function slotMascotSize(slot) {
    if (slot.closest(".compact-session-dot")) return 22;
    if (slot.closest(".approval-kicker")) return 20;
    if (slot.closest(".approval-tab")) return 25;
    if (slot.closest(".agent-card-mascot")) return 48;
    if (slot.classList.contains("agent-mascot-slot")) return 79;
    if (slot.closest(".approval-preview")) return 25;
    return 28;
  }

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
    const prompt = mixHex(accent, "#ffffff", 0.55);
    return {
      body: accent,
      bodyTop: mixHex(accent, "#ffffff", 0.18),
      dark,
      screen: "#0c1018",
      prompt,
      eye: prompt,
      blush: mixHex(accent, "#ffffff", 0.45),
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
          ${bang}
        </g>
      </svg>
    </span>`;
  }

  function renderCodex(palette, mood) {
    return `<span class="codex is-${mood}" aria-hidden="true">
      <svg class="codex-svg" viewBox="${VIEWBOX}" preserveAspectRatio="xMidYMid meet" shape-rendering="crispEdges">
        <ellipse class="codex-shadow" cx="68" cy="92" rx="32" ry="4" fill="rgba(0,0,0,0.18)"/>
        <g class="codex-body">
          <rect class="codex-chassis" x="24" y="8" width="64" height="48" fill="${palette.body}"/>
          <rect x="24" y="8" width="64" height="8" fill="${palette.bodyTop}"/>
          <rect class="codex-screen" x="32" y="18" width="48" height="28" fill="${palette.screen}"/>
          <rect class="codex-screen-glow" x="32" y="18" width="48" height="28" fill="${palette.prompt}"/>
          <g class="codex-prompt">
            <rect x="43" y="24" width="8" height="4" fill="${palette.prompt}"/>
            <rect x="47" y="28" width="8" height="4" fill="${palette.prompt}"/>
            <rect x="43" y="32" width="8" height="4" fill="${palette.prompt}"/>
            <rect class="codex-cursor" x="57" y="32" width="12" height="4" fill="${palette.prompt}"/>
          </g>
          <rect class="codex-leg codex-leg-0" x="36" y="56" width="12" height="16" fill="${palette.dark}"/>
          <rect class="codex-leg codex-leg-1" x="64" y="56" width="12" height="16" fill="${palette.dark}"/>
        </g>
      </svg>
    </span>`;
  }

  function renderAgent(agentId, moodOverride, size) {
    const config = AGENTS[agentId];
    if (!config) return "";
    const mood = moodOverride || config.mood;

    if (config.type === "cursor" && window.AtollCursorMascot) {
      return window.AtollCursorMascot.render(mood, size || 79, false);
    }

    if (config.type === "codex") {
      return renderCodex(codexPalette(config.accent, config.accentDark), mood);
    }

    return renderClawd(clawdPalette(config.accent, config.accentDark), mood);
  }

  function init() {
    document.querySelectorAll(".mascot-slot[data-agent], .agent-mascot-slot[data-agent]").forEach((slot) => {
      const agentId = slot.dataset.agent;
      const config = AGENTS[agentId];
      const mood = slot.dataset.mood || config?.mood || "calm";
      const size = slotMascotSize(slot);

      if (config?.type === "cursor" && window.AtollCursorMascot?.mount) {
        window.AtollCursorMascot.mount(slot, mood, size);
      } else {
        slot.innerHTML = renderAgent(agentId, slot.dataset.mood || undefined, size);
      }

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
