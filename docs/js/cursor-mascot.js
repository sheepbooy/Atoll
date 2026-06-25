(function () {
  const EYE = "#1a1a1a";
  const EYE_W = 5;
  const EYE_H = 11;
  const EYE_H_WIDE = 12;
  const EYE_BLINK_W = 8;
  const EYE_BLINK_H = 2;
  const VIEWBOX = { x: -20, y: -34, w: 152, h: 136 };
  const ASPECT = VIEWBOX.w / VIEWBOX.h;
  const CUBE_SCALE = 1.58;
  const ANCHOR = { x: 60, y: 38 };
  const CUBE_TRANSFORM = `translate(${ANCHOR.x} ${ANCHOR.y}) scale(${CUBE_SCALE}) translate(${-ANCHOR.x} ${-ANCHOR.y})`;
  const ACCENT = "#a78bfa";
  const ACCENT_DARK = "#7c5fd4";

  const CUBE = {
    top: "32,18 60,4 88,18 60,32",
    left: "32,18 60,32 60,68 32,54",
    front: "60,32 88,18 88,54 60,68",
    facet: "88,18 74,28 88,42",
    groove: { x1: 74, y1: 28, x2: 64, y2: 60 },
  };

  const FLOATING_EYES = [
    { x: 47, y: 33 },
    { x: 73, y: 33 },
  ];
  const FLOATING_MOUTH = { x: 60, y: 39 };

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

  function defaultPalette() {
    return {
      top: "#ede9fe",
      left: "#c4b5fd",
      front: "#8b6fd8",
      facet: "#ddd6fe",
      groove: "#6d4fc9",
      outline: "#b8a4f8",
      blush: "#f0e8ff",
      sparkle: "#f5f3ff",
      sweat: "#c4b5fd",
      glowInner: "rgba(196, 181, 253, 0.75)",
      glowOuter: "rgba(124, 95, 212, 0.28)",
    };
  }

  function derivePalette(accent, accentDark) {
    const dark = accentDark || mixHex(accent, "#000000", 0.35);
    const rgb = parseHex(accent);
    return {
      top: mixHex(accent, "#ffffff", 0.42),
      left: mixHex(accent, "#ffffff", 0.18),
      front: dark,
      facet: mixHex(accent, "#ffffff", 0.55),
      groove: mixHex(dark, "#000000", 0.28),
      outline: mixHex(accent, "#ffffff", 0.35),
      blush: mixHex(accent, "#ffffff", 0.45),
      sparkle: mixHex(accent, "#ffffff", 0.62),
      sweat: mixHex(accent, "#ffffff", 0.35),
      glowInner: rgb
        ? `rgba(${Math.min(rgb[0] + 40, 255)}, ${Math.min(rgb[1] + 40, 255)}, ${Math.min(rgb[2] + 40, 255)}, 0.75)`
        : "rgba(196, 181, 253, 0.75)",
      glowOuter: rgb
        ? `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, 0.28)`
        : "rgba(124, 95, 212, 0.28)",
    };
  }

  function paletteFor(mood) {
    if (mood === "dead") {
      return {
        top: "#a4a8ac",
        left: "#909498",
        front: "#787c80",
        facet: "#b0b4b8",
        groove: "#585c60",
        outline: "#9aa0a4",
        blush: "#b0b4b8",
        sparkle: "#c8ccd0",
        sweat: "#a0a4a8",
        glowInner: "rgba(140, 148, 152, 0.45)",
        glowOuter: "rgba(80, 88, 92, 0.28)",
      };
    }
    if (mood === "worried") {
      const base = defaultPalette();
      return {
        ...base,
        top: "#7cb97c",
        left: mixHex("#7cb97c", "#ffffff", 0.15),
        front: "#4a8a4a",
        facet: mixHex("#7cb97c", "#ffffff", 0.35),
        groove: mixHex("#4a8a4a", "#000000", 0.2),
        outline: mixHex("#7cb97c", "#ffffff", 0.25),
        blush: "#d4f5d4",
        sparkle: "#e8ffe8",
        sweat: "#a8d8a8",
        glowInner: "rgba(212, 245, 212, 0.75)",
        glowOuter: "rgba(124, 185, 124, 0.28)",
      };
    }
    return derivePalette(ACCENT, ACCENT_DARK);
  }

  function eyeFloatPad(x, y) {
    return `<ellipse cx="${x}" cy="${y + 0.5}" rx="4.8" ry="6.8" fill="rgba(255,255,255,0.2)"/>`;
  }

  function wrapEye(x, y, inner) {
    return `<g class="cursor-mascot-eye-float">${eyeFloatPad(x, y)}${inner}</g>`;
  }

  function verticalEye(x, y, w = EYE_W, h = EYE_H, fill = EYE) {
    return `<rect x="${x - w / 2}" y="${y - h / 2}" width="${w}" height="${h}" fill="${fill}"/>`;
  }

  function blinkEye(x, y) {
    return `<rect x="${x - EYE_BLINK_W / 2}" y="${y - EYE_BLINK_H / 2}" width="${EYE_BLINK_W}" height="${EYE_BLINK_H}" fill="${EYE}"/>`;
  }

  function deadEye(x, y) {
    return `
      <line x1="${x - 4.5}" y1="${y - 5.5}" x2="${x + 4.5}" y2="${y + 5.5}" stroke="#e8eaed" stroke-width="2.8" stroke-linecap="round"/>
      <line x1="${x + 4.5}" y1="${y - 5.5}" x2="${x - 4.5}" y2="${y + 5.5}" stroke="#e8eaed" stroke-width="2.8" stroke-linecap="round"/>`;
  }

  function brow(x, y, tilt, index) {
    const by = y - EYE_H / 2 - 5;
    const w = 7;
    const angle = tilt === "sad" ? (index === 0 ? -18 : 18) : index === 0 ? 18 : -18;
    return `<rect x="${x - w / 2}" y="${by}" width="${w}" height="2" fill="${EYE}" rx="0.4" transform="rotate(${angle} ${x} ${by})"/>`;
  }

  function face(palette) {
    return `
      <polygon points="${CUBE.left}" fill="${palette.left}" stroke="${palette.outline}" stroke-width="2" stroke-linejoin="round"/>
      <polygon points="${CUBE.front}" fill="${palette.front}" stroke="${palette.outline}" stroke-width="2" stroke-linejoin="round"/>
      <polygon points="${CUBE.top}" fill="${palette.top}" stroke="${palette.outline}" stroke-width="2" stroke-linejoin="round"/>
      <polygon points="${CUBE.facet}" fill="${palette.facet}" stroke="${palette.outline}" stroke-width="1.5" stroke-linejoin="round"/>
      <line x1="${CUBE.groove.x1}" y1="${CUBE.groove.y1}" x2="${CUBE.groove.x2}" y2="${CUBE.groove.y2}"
        stroke="${palette.groove}" stroke-width="2.4" stroke-linecap="round"/>
    `;
  }

  function renderExpression(mood, blinking) {
    const [left, right] = FLOATING_EYES;

    if (mood === "dead") {
      return wrapEye(left.x, left.y, deadEye(left.x, left.y))
        + wrapEye(right.x, right.y, deadEye(right.x, right.y));
    }

    if (mood === "happy") {
      const mouth = FLOATING_MOUTH;
      return `<g class="cursor-mascot-eyes">
        ${wrapEye(left.x, left.y, blinkEye(left.x, left.y))}
        ${wrapEye(right.x, right.y, blinkEye(right.x, right.y))}
        <rect x="${mouth.x - 4}" y="${mouth.y}" width="8" height="2" fill="${EYE}" rx="0.5" opacity="0.85"/>
      </g>`;
    }

    if (mood === "sleeping") {
      return `<g class="cursor-mascot-eyes">
        ${wrapEye(left.x, left.y, blinkEye(left.x, left.y))}
        ${wrapEye(right.x, right.y, blinkEye(right.x, right.y))}
      </g>`;
    }

    if (mood === "worried") {
      return `<g class="cursor-mascot-eyes">
        ${wrapEye(left.x, left.y, verticalEye(left.x, left.y) + brow(left.x, left.y, "worried", 0))}
        ${wrapEye(right.x, right.y, verticalEye(right.x, right.y) + brow(right.x, right.y, "worried", 1))}
      </g>`;
    }

    if (mood === "sad") {
      return `<g class="cursor-mascot-eyes">
        ${wrapEye(left.x, left.y, verticalEye(left.x, left.y) + brow(left.x, left.y, "sad", 0))}
        ${wrapEye(right.x, right.y, verticalEye(right.x, right.y) + brow(right.x, right.y, "sad", 1))}
      </g>`;
    }

    const draw = (point) =>
      blinking
        ? blinkEye(point.x, point.y)
        : verticalEye(point.x, point.y, EYE_W, mood === "alert" ? EYE_H_WIDE : EYE_H);

    return `<g class="cursor-mascot-eyes">
      ${wrapEye(left.x, left.y, draw(left))}
      ${wrapEye(right.x, right.y, draw(right))}
    </g>`;
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
      <span class="cursor-mascot is-${mood}" style="width:${width}px;height:${size}px;--glow-inner:${palette.glowInner};--glow-outer:${palette.glowOuter}">
        <svg class="cursor-mascot-svg" width="100%" height="100%" viewBox="${VIEWBOX.x} ${VIEWBOX.y} ${VIEWBOX.w} ${VIEWBOX.h}" preserveAspectRatio="xMidYMid meet">
          <g class="cursor-mascot-figure" transform="${CUBE_TRANSFORM}">
            <ellipse cx="60" cy="78" rx="30" ry="4.5" fill="rgba(0,0,0,0.22)"/>
            <g class="cursor-mascot-cube" shape-rendering="crispEdges">${face(palette)}</g>
            ${renderExpression(mood, blinking)}
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
