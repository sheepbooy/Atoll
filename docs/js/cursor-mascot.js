(function () {
  const VIEWBOX = { x: -20, y: -34, w: 152, h: 136 };
  const ASPECT = VIEWBOX.w / VIEWBOX.h;
  const CUBE = {
    bottom: { fill: "#72716D", d: "M483.395 490.5L566 538.297C565.493 539.178 564.757 539.93 563.845 540.456L486.636 585.13C484.632 586.29 482.159 586.29 480.154 585.13L402.945 540.456C402.034 539.93 401.297 539.178 400.79 538.297L483.395 490.5Z" },
    left: { fill: "#55544F", d: "M483.395 395V490.5L400.79 538.297C400.282 537.416 400 536.398 400 535.346V445.654C400 443.545 401.122 441.6 402.945 440.544L480.15 395.87C481.154 395.29 482.273 395 483.391 395H483.395Z" },
    right: { fill: "#43413C", d: "M565.996 442.703C565.489 441.822 564.752 441.07 563.841 440.544L486.632 395.87C485.632 395.29 484.513 395 483.395 395V490.5L566 538.297C566.507 537.416 566.789 536.398 566.789 535.346V445.654C566.789 444.598 566.511 443.588 566 442.703H565.996Z" },
    stem: { fill: "#D6D5D2", d: "M560.218 446.049C560.686 446.858 560.751 447.896 560.218 448.82L485.235 578.974C484.732 579.855 483.392 579.493 483.392 578.479V492.713C483.392 492.029 483.209 491.37 482.877 490.794L560.215 446.045H560.218V446.049Z" },
    head: { fill: "#FFFFFF", d: "M560.218 446.049L482.88 490.797C482.552 490.224 482.073 489.737 481.48 489.394L407.369 446.511C406.49 446.006 406.851 444.663 407.862 444.663H557.824C558.889 444.663 559.754 445.239 560.218 446.049Z" },
  };
  const CUBE_DEAD = {
    bottom: "#7a7a7a",
    left: "#5c5c5c",
    right: "#484848",
    stem: "#c8c8c8",
    head: "#e8e8e8",
  };
  const CUBE_SICK = {
    bottom: "#6d8a6d",
    left: "#4f6b4f",
    right: "#3c523c",
    stem: "#c5d6c5",
    head: "#f4fff4",
  };

  function paletteFor(mood) {
    if (mood === "dead") return { ...CUBE_DEAD, sparkle: "#c8ccd0", sweat: "#a0a4a8" };
    if (mood === "worried") return { ...CUBE_SICK, sparkle: "#e8ffe8", sweat: "#a8d8a8" };
    return {
      bottom: CUBE.bottom.fill,
      left: CUBE.left.fill,
      right: CUBE.right.fill,
      stem: CUBE.stem.fill,
      head: CUBE.head.fill,
      sparkle: "#f5f3ff",
      sweat: "#c4b5fd",
    };
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

  function cubeMarkup(palette) {
    return `
      <g class="cursor-mascot-cube cursor-mascot-mark">
        <g transform="translate(56 40) scale(0.345) translate(-482.5 -491.5)">
          <path fill="${palette.bottom}" d="${CUBE.bottom.d}"/>
          <path fill="${palette.left}" d="${CUBE.left.d}"/>
          <path fill="${palette.right}" d="${CUBE.right.d}"/>
          <path fill="${palette.stem}" d="${CUBE.stem.d}"/>
          <path class="cursor-mascot-pointer-head" fill="${palette.head}" d="${CUBE.head.d}"/>
        </g>
      </g>`;
  }

  function render(mood, size) {
    const palette = paletteFor(mood);
    const width = size * ASPECT;

    return `
      <span class="cursor-mascot is-${mood}" style="width:${width}px;height:${size}px">
        <svg class="cursor-mascot-svg" width="100%" height="100%" viewBox="${VIEWBOX.x} ${VIEWBOX.y} ${VIEWBOX.w} ${VIEWBOX.h}" preserveAspectRatio="xMidYMid meet">
          <ellipse class="cursor-mascot-shadow" cx="68" cy="92" rx="32" ry="4" fill="rgba(0,0,0,0.18)"/>
          <g class="cursor-mascot-body">
            ${cubeMarkup(palette)}
            ${extras(mood, palette)}
          </g>
        </svg>
      </span>`;
  }

  function mount(element, mood, size) {
    if (!element) return;
    element.innerHTML = render(mood, size);
  }

  window.AtollCursorMascot = { render, mount, paletteFor };
})();
