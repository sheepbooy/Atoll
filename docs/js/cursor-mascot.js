(function () {
  const EYE = "#1a1a1a";
  const DEAD_EYE = "#050505";
  const SICK = "#7cb97c";
  const SICK_DARK = "#5a8b5a";
  const VIEWBOX = { x: -20, y: -34, w: 152, h: 136 };
  const ASPECT = VIEWBOX.w / VIEWBOX.h;
  const ACCENT = "#a78bfa";
  const ACCENT_DARK = "#7c5fd4";
  const LEFT_X = 40;
  const RIGHT_X = 64;
  const EYE_Y = 18;
  const EYE_W = 8;
  const EYE_H = 16;

  function parseHex(hex) {
    const value = Number.parseInt(hex.replace("#", ""), 16);
    if (Number.isNaN(value)) return null;
    return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
  }

  function mixHex(a, b, weight) {
    const left = parseHex(a);
    const right = parseHex(b);
    if (!left || !right) return a;
    const mix = (l, r) => Math.round(l * (1 - weight) + r * weight);
    const rgb = [mix(left[0], right[0]), mix(left[1], right[1]), mix(left[2], right[2])];
    return `#${rgb.map((channel) => channel.toString(16).padStart(2, "0")).join("")}`;
  }

  function paletteFor(mood) {
    if (mood === "dead") {
      return {
        front: "#8a8a8a",
        top: "#9a9a9a",
        side: "#666666",
        blush: "#b0b4b8",
        sparkle: "#c8ccd0",
        sweat: "#a0a4a8",
        eye: DEAD_EYE,
      };
    }
    if (mood === "worried") {
      return {
        front: SICK,
        top: mixHex(SICK, "#ffffff", 0.18),
        side: SICK_DARK,
        blush: "#d4f5d4",
        sparkle: "#e8ffe8",
        sweat: "#a8d8a8",
        eye: EYE,
      };
    }
    return {
      front: ACCENT,
      top: mixHex(ACCENT, "#ffffff", 0.28),
      side: ACCENT_DARK,
      blush: mixHex(ACCENT, "#ffffff", 0.45),
      sparkle: mixHex(ACCENT, "#ffffff", 0.62),
      sweat: mixHex(ACCENT, "#ffffff", 0.35),
      eye: EYE,
    };
  }

  function face(mood, blinking, eyeFill, blush) {
    const eyeH = blinking ? 2.4 : EYE_H;
    let eyes = "";
    if (mood === "dead") {
      eyes = `
        <line x1="${LEFT_X - 2}" y1="${EYE_Y - 2}" x2="${LEFT_X + EYE_W + 2}" y2="${EYE_Y + EYE_H + 2}" stroke="${eyeFill}" stroke-width="3.2" stroke-linecap="round"/>
        <line x1="${LEFT_X + EYE_W + 2}" y1="${EYE_Y - 2}" x2="${LEFT_X - 2}" y2="${EYE_Y + EYE_H + 2}" stroke="${eyeFill}" stroke-width="3.2" stroke-linecap="round"/>
        <line x1="${RIGHT_X - 2}" y1="${EYE_Y - 2}" x2="${RIGHT_X + EYE_W + 2}" y2="${EYE_Y + EYE_H + 2}" stroke="${eyeFill}" stroke-width="3.2" stroke-linecap="round"/>
        <line x1="${RIGHT_X + EYE_W + 2}" y1="${EYE_Y - 2}" x2="${RIGHT_X - 2}" y2="${EYE_Y + EYE_H + 2}" stroke="${eyeFill}" stroke-width="3.2" stroke-linecap="round"/>`;
    } else if (mood === "worried") {
      eyes = `
        <line x1="${LEFT_X}" y1="${EYE_Y}" x2="${LEFT_X + EYE_W}" y2="${EYE_Y + EYE_H}" stroke="${eyeFill}" stroke-width="2.4" stroke-linecap="round"/>
        <line x1="${LEFT_X + EYE_W}" y1="${EYE_Y}" x2="${LEFT_X}" y2="${EYE_Y + EYE_H}" stroke="${eyeFill}" stroke-width="2.4" stroke-linecap="round"/>
        <line x1="${RIGHT_X}" y1="${EYE_Y}" x2="${RIGHT_X + EYE_W}" y2="${EYE_Y + EYE_H}" stroke="${eyeFill}" stroke-width="2.4" stroke-linecap="round"/>
        <line x1="${RIGHT_X + EYE_W}" y1="${EYE_Y}" x2="${RIGHT_X}" y2="${EYE_Y + EYE_H}" stroke="${eyeFill}" stroke-width="2.4" stroke-linecap="round"/>`;
    } else if (mood === "sleeping") {
      eyes = `<rect x="${LEFT_X}" y="${EYE_Y + 4}" width="${EYE_W + 1.6}" height="2.4" fill="${eyeFill}"/><rect x="${RIGHT_X}" y="${EYE_Y + 4}" width="${EYE_W + 1.6}" height="2.4" fill="${eyeFill}"/>`;
    } else {
      eyes = `<rect x="${LEFT_X}" y="${EYE_Y}" width="${EYE_W}" height="${eyeH}" fill="${eyeFill}"/><rect x="${RIGHT_X}" y="${EYE_Y}" width="${EYE_W}" height="${eyeH}" fill="${eyeFill}"/>`;
    }

    const brows =
      mood === "sad"
        ? `<rect x="${LEFT_X - 4}" y="${EYE_Y - 4}" width="12" height="2.4" fill="${eyeFill}" transform="rotate(-15 ${LEFT_X + 2} ${EYE_Y - 2.8})"/>
           <rect x="${RIGHT_X}" y="${EYE_Y - 4}" width="12" height="2.4" fill="${eyeFill}" transform="rotate(15 ${RIGHT_X + 6} ${EYE_Y - 2.8})"/>`
        : "";

    const cheeks =
      mood === "happy" || mood === "alert"
        ? `<ellipse cx="${LEFT_X - 6}" cy="46" rx="6" ry="3.2" fill="${blush}" opacity="0.65"/>
           <ellipse cx="${RIGHT_X + EYE_W + 6}" cy="46" rx="6" ry="3.2" fill="${blush}" opacity="0.65"/>`
        : "";

    return eyes + brows + cheeks;
  }

  function star(className, fill) {
    return `<polygon class="${className}" points="0,-6 1.6,-1.6 6,0 1.6,1.6 0,6 -1.6,1.6 -6,0 -1.6,-1.6" fill="${fill}"/>`;
  }

  function extras(mood, palette) {
    if (mood === "alert") {
      return `<g class="cursor-mascot-bang">
        <rect x="53" y="-30" width="6" height="14" rx="1.5" fill="#f8dda0"/>
        <rect x="53" y="-13" width="6" height="5" rx="1.5" fill="#f8dda0"/>
      </g>`;
    }
    if (mood === "happy") {
      return `
        <path class="cursor-mascot-heart" d="M56 -14 C 51 -22 41 -17 56 -4 C 71 -17 61 -22 56 -14 Z" fill="${palette.sparkle}"/>
        <g transform="translate(-6 12)">${star("cursor-mascot-star cursor-mascot-star-0", palette.sparkle)}</g>
        <g transform="translate(118 6)">${star("cursor-mascot-star cursor-mascot-star-1", palette.sparkle)}</g>
        <g transform="translate(96 -24)">${star("cursor-mascot-star cursor-mascot-star-2", palette.sparkle)}</g>`;
    }
    if (mood === "sleeping") {
      return `<g class="cursor-mascot-zzz" fill="#aab4ff" font-family="ui-monospace,monospace" font-weight="700">
        <text class="cursor-mascot-z cursor-mascot-z-0" x="104" y="-6" font-size="16">z</text>
        <text class="cursor-mascot-z cursor-mascot-z-1" x="116" y="-16" font-size="20">z</text>
        <text class="cursor-mascot-z cursor-mascot-z-2" x="128" y="-28" font-size="24">z</text>
      </g>`;
    }
    if (mood === "worried") {
      return `<g transform="translate(108 -4)"><path class="cursor-mascot-sweat" d="M4 0 C 8 7 0 7 4 0 Z" fill="${palette.sweat}"/></g>`;
    }
    return "";
  }

  function render(mood, size, blinking) {
    const palette = paletteFor(mood);
    const width = size * ASPECT;

    return `
      <span class="cursor-mascot is-${mood}" style="width:${width}px;height:${size}px">
        <svg class="cursor-mascot-svg" width="100%" height="100%" viewBox="${VIEWBOX.x} ${VIEWBOX.y} ${VIEWBOX.w} ${VIEWBOX.h}" preserveAspectRatio="xMidYMid meet" shape-rendering="crispEdges">
          <ellipse class="cursor-mascot-shadow" cx="68" cy="92" rx="32" ry="4" fill="rgba(0,0,0,0.18)"/>
          <g class="cursor-mascot-body">
            <g class="cursor-mascot-claw cursor-mascot-claw-left cursor-mascot-pointer">
              <rect x="16" y="-14" width="8" height="8" fill="${palette.front}"/>
              <rect x="16" y="-6" width="16" height="8" fill="${palette.front}"/>
              <rect x="16" y="2" width="8" height="12" fill="${palette.front}"/>
              <rect x="24" y="2" width="8" height="8" fill="${palette.front}"/>
            </g>
            <rect class="cursor-mascot-claw cursor-mascot-claw-right" x="100" y="28" width="10" height="14" fill="${palette.side}"/>
            <g class="cursor-mascot-cube">
              <rect class="cursor-mascot-face-front" x="28" y="6" width="56" height="56" fill="${palette.front}"/>
              <rect class="cursor-mascot-face-top" x="28" y="6" width="56" height="10" fill="${palette.top}"/>
              <rect class="cursor-mascot-face-side" x="84" y="14" width="16" height="48" fill="${palette.side}"/>
            </g>
            ${face(mood, blinking, palette.eye, palette.blush)}
            <rect class="cursor-mascot-leg cursor-mascot-leg-0" x="38" y="62" width="12" height="16" fill="${palette.side}"/>
            <rect class="cursor-mascot-leg cursor-mascot-leg-1" x="66" y="62" width="12" height="16" fill="${palette.side}"/>
            ${extras(mood, palette)}
          </g>
        </svg>
      </span>`;
  }

  const blinkTimers = new WeakMap();

  function mount(element, mood, size) {
    if (!element) return;
    const existing = blinkTimers.get(element);
    if (existing) {
      window.clearTimeout(existing);
      blinkTimers.delete(element);
    }

    element.innerHTML = render(mood, size, false);

    if (mood === "sleeping" || mood === "dead") {
      return;
    }

    const loop = () => {
      element.innerHTML = render(mood, size, true);
      const timer = window.setTimeout(() => {
        element.innerHTML = render(mood, size, false);
        const nextTimer = window.setTimeout(loop, 3000 + Math.random() * 2500);
        blinkTimers.set(element, nextTimer);
      }, 150);
      blinkTimers.set(element, timer);
    };

    const startTimer = window.setTimeout(loop, 2500 + Math.random() * 2500);
    blinkTimers.set(element, startTimer);
  }

  window.AtollCursorMascot = { render, mount, paletteFor };
})();
