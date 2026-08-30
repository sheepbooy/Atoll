(function () {
  const VIEWBOX = "-20 -34 152 136";
  const LEGS = [16, 30.4, 72, 86.4];
  const EYE = "#1a1a1a";
  const CODEX_PATH =
    "M8.086.457a6.105 6.105 0 013.046-.415c1.333.153 2.521.72 3.564 1.7a.117.117 0 00.107.029c1.408-.346 2.762-.224 4.061.366l.063.03.154.076c1.357.703 2.33 1.77 2.918 3.198.278.679.418 1.388.421 2.126a5.655 5.655 0 01-.18 1.631.167.167 0 00.04.155 5.982 5.982 0 011.578 2.891c.385 1.901-.01 3.615-1.183 5.14l-.182.22a6.063 6.063 0 01-2.934 1.851.162.162 0 00-.108.102c-.255.736-.511 1.364-.987 1.992-1.199 1.582-2.962 2.462-4.948 2.451-1.583-.008-2.986-.587-4.21-1.736a.145.145 0 00-.14-.032c-.518.167-1.04.191-1.604.185a5.924 5.924 0 01-2.595-.622 6.058 6.058 0 01-2.146-1.781c-.203-.269-.404-.522-.551-.821a7.74 7.74 0 01-.495-1.283 6.11 6.11 0 01-.017-3.064.166.166 0 00.008-.074.115.115 0 00-.037-.064 5.958 5.958 0 01-1.38-2.202 5.196 5.196 0 01-.333-1.589 6.915 6.915 0 01.188-2.132c.45-1.484 1.309-2.648 2.577-3.493.282-.188.55-.334.802-.438.286-.12.573-.22.861-.304a.129.129 0 00.087-.087A6.016 6.016 0 015.635 2.31C6.315 1.464 7.132.846 8.086.457zm-.804 7.85a.848.848 0 00-1.473.842l1.694 2.965-1.688 2.848a.849.849 0 001.46.864l1.94-3.272a.849.849 0 00.007-.854l-1.94-3.393zm5.446 6.24a.849.849 0 000 1.695h4.848a.849.849 0 000-1.696h-4.848z";

  const AGENTS = {
    claude: { type: "clawd", mood: "calm" },
    codex: {
      type: "codex",
      mood: "calm",
    },
    gemini: {
      type: "gemini",
      mood: "calm",
    },
    cursor: {
      type: "cursor",
      mood: "calm",
    },
    zcode: {
      type: "zcode",
      mood: "calm",
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
      return [(value >> 16) & 255, (value >> 8) & 255, (value >> 0) & 255];
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

  function codexFill(mood) {
    if (mood === "dead") return "#8a8a8a";
    if (mood === "worried") return "#7cb97c";
    if (mood === "sleeping") return "#a8b0b4";
    return "#f4f4f4";
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

  function renderCodex(mood) {
    const fill = codexFill(mood);
    return `<span class="codex is-${mood}" aria-hidden="true">
      <svg class="codex-svg" viewBox="${VIEWBOX}" preserveAspectRatio="xMidYMid meet">
        <ellipse class="codex-shadow" cx="68" cy="92" rx="32" ry="4" fill="rgba(0,0,0,0.18)"/>
        <g class="codex-body">
          <g class="codex-mark">
            <g transform="translate(56 38) scale(2.7) translate(-12 -12)">
              <path fill="${fill}" fill-rule="evenodd" clip-rule="evenodd" d="${CODEX_PATH}"/>
            </g>
          </g>
        </g>
      </svg>
    </span>`;
  }

  const ZCODE_TILE_PATH =
    "M5.3 .6h13.4a5.3 5.3 0 015.3 5.3v12.2a5.3 5.3 0 01-5.3 5.3H5.3A5.3 5.3 0 010 18.1V5.9A5.3 5.3 0 015.3 .6Z";
  const ZCODE_Z_PATH = "M5.2 5.8H18.8V8L9.2 16H18.8V18.2H5.2V16L14.8 8H5.2Z";
  const GEMINI_SPARK_PATH =
    "M11.04 19.32Q12 21.51 12 24q0-2.49.93-4.68.96-2.19 2.58-3.81t3.81-2.55Q21.51 12 24 12q-2.49 0-4.68-.93a12.3 12.3 0 0 1-3.81-2.58 12.3 12.3 0 0 1-2.58-3.81Q12 2.49 12 0q0 2.49-.96 4.68-.93 2.19-2.55 3.81a12.3 12.3 0 0 1-3.81 2.58Q2.49 12 0 12q2.49 0 4.68.96 2.19.93 3.81 2.55t2.55 3.81";

  function renderGemini(mood) {
    return `<span class="gemini is-${mood}" aria-hidden="true">
      <svg class="gemini-svg" viewBox="${VIEWBOX}" preserveAspectRatio="xMidYMid meet">
        <defs>
          <linearGradient id="gemini-spark-gradient" x1="0" y1="0" x2="0.9" y2="1">
            <stop offset="0" stop-color="#4796E3"/>
            <stop offset="0.5" stop-color="#9177C7"/>
            <stop offset="1" stop-color="#CA6673"/>
          </linearGradient>
        </defs>
        <ellipse class="gemini-shadow" cx="68" cy="92" rx="32" ry="4" fill="rgba(0,0,0,0.18)"/>
        <g class="gemini-body">
          <g transform="translate(56 39) scale(3.15) translate(-12 -12)">
            <path fill="url(#gemini-spark-gradient)" d="${GEMINI_SPARK_PATH}"/>
          </g>
        </g>
      </svg>
    </span>`;
  }

  function renderZcode(mood) {
    return `<span class="zcode is-${mood}" aria-hidden="true">
      <svg class="zcode-svg" viewBox="${VIEWBOX}" preserveAspectRatio="xMidYMid meet">
        <defs>
          <linearGradient id="zcode-tile-gradient" x1="0" y1="0" x2="0.35" y2="1">
            <stop offset="0" stop-color="#58c7f5"/>
            <stop offset="1" stop-color="#1f8fd0"/>
          </linearGradient>
        </defs>
        <ellipse class="zcode-shadow" cx="68" cy="92" rx="32" ry="4" fill="rgba(0,0,0,0.18)"/>
        <g class="zcode-body">
          <g transform="translate(56 39) scale(3.15) translate(-12 -12)">
            <path fill="url(#zcode-tile-gradient)" d="${ZCODE_TILE_PATH}"/>
            <path fill="#ffffff" transform="translate(2 0) skewX(-9.5)" d="${ZCODE_Z_PATH}"/>
          </g>
        </g>
      </svg>
    </span>`;
  }

  function renderAgent(agentId, moodOverride, size) {
    const config = AGENTS[agentId];
    if (!config) return "";
    const mood = moodOverride || config.mood;

    if (config.type === "cursor" && window.AtollCursorMascot) {
      return window.AtollCursorMascot.render(mood, size || 79);
    }

    if (config.type === "codex") {
      return renderCodex(mood);
    }

    if (config.type === "zcode") {
      return renderZcode(mood);
    }

    if (config.type === "gemini") {
      return renderGemini(mood);
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
