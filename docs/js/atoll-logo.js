(function () {
  const VIEWBOX = "-16 -36 96 108";
  const ASPECT = 96 / 108;
  const BODY = "#38BDD8";
  const TOP = "#5FD8EC";
  const LIMB = "#2A8FA8";
  const EYE = "#1a1a1a";

  function renderBody(showLimbs, closedEyes) {
    const eyeY = closedEyes ? 28 : 24;
    const eyeH = closedEyes ? 2 : 11;
    const legs = showLimbs
      ? `<rect class="atoll-limb atoll-leg-left" x="20" y="44" width="6" height="12" fill="${LIMB}"/>
         <rect class="atoll-limb atoll-leg-right" x="38" y="44" width="6" height="12" fill="${LIMB}"/>`
      : "";
    return `<g class="atoll-body-group">
      ${legs}
      <rect x="8" y="18" width="48" height="26" fill="${BODY}"/>
      <rect x="8" y="18" width="48" height="5" fill="${TOP}"/>
      <rect class="atoll-eye atoll-eye-left" x="19" y="${eyeY}" width="5" height="${eyeH}" fill="${EYE}"/>
      <rect class="atoll-eye atoll-eye-right" x="38" y="${eyeY}" width="5" height="${eyeH}" fill="${EYE}"/>
    </g>`;
  }

  function renderDesk() {
    return `<g class="atoll-prop atoll-desk">
      <rect x="14" y="58" width="36" height="3" fill="#5A5A6A"/>
      <rect x="16" y="61" width="3" height="7" fill="#4A4A5A"/>
      <rect x="45" y="61" width="3" height="7" fill="#4A4A5A"/>
      <rect x="18" y="40" width="28" height="18" fill="#2A2A3A"/>
      <rect class="atoll-screen" x="20" y="42" width="24" height="14" fill="#0D1117"/>
      <rect class="atoll-code-line atoll-code-0" x="22" y="45" width="10" height="1.5" fill="#6BE088"/>
      <rect class="atoll-code-line atoll-code-1" x="22" y="48" width="16" height="1.5" fill="#68B8F8"/>
      <rect class="atoll-code-line atoll-code-2" x="22" y="51" width="8" height="1.5" fill="#F8C868"/>
      <rect class="atoll-code-line atoll-code-3" x="22" y="54" width="12" height="1.5" fill="#E080C0"/>
      <rect class="atoll-cursor" x="22" y="54" width="1.5" height="2" fill="#58A6FF"/>
      <rect x="16" y="56" width="32" height="2" fill="#3A3A4A"/>
      <rect class="atoll-key atoll-key-0" x="18" y="57" width="5" height="2" fill="#6A6A7A"/>
      <rect class="atoll-key atoll-key-1" x="24" y="57" width="5" height="2" fill="#6A6A7A"/>
      <rect class="atoll-key atoll-key-2" x="30" y="57" width="5" height="2" fill="#6A6A7A"/>
      <rect class="atoll-key atoll-key-3" x="36" y="57" width="5" height="2" fill="#6A6A7A"/>
      <rect class="atoll-key atoll-key-4" x="42" y="57" width="5" height="2" fill="#6A6A7A"/>
    </g>`;
  }

  function renderThought() {
    return `<g class="atoll-prop atoll-thought">
      <circle cx="52" cy="2" r="3.5" fill="#fff" fill-opacity="0.95"/>
      <circle cx="57" cy="-5" r="2.5" fill="#fff" fill-opacity="0.95"/>
      <rect x="26" y="-30" width="38" height="22" rx="9" fill="#fff" fill-opacity="0.95"/>
      <text class="atoll-think-mark atoll-think-q" x="40" y="-13" text-anchor="middle" font-size="18" font-weight="800" fill="#7C6BC4" font-family="ui-monospace,monospace">?</text>
    </g>`;
  }

  function renderNap() {
    return `<g class="atoll-prop atoll-sleep-cap">
        <polygon points="22,16 54,16 58,-6" fill="#6B5B9A"/>
        <rect x="20" y="14" width="36" height="5" fill="#8070B0"/>
        <circle class="atoll-cap-pom" cx="60" cy="-8" r="4" fill="#E8E0F0"/>
      </g>
      <g class="atoll-prop atoll-zzz" fill="#aab4ff" font-family="ui-monospace,monospace" font-weight="700">
        <text class="atoll-z atoll-z-0" x="48" y="8" font-size="12">z</text>
        <text class="atoll-z atoll-z-1" x="56" y="-4" font-size="15">z</text>
        <text class="atoll-z atoll-z-2" x="64" y="-18" font-size="18">z</text>
      </g>`;
  }

  function renderAtoll(activity) {
    let body;
    let props = "";

    switch (activity) {
      case "thinking":
        body = renderBody(true, false);
        props = renderThought();
        break;
      case "coding":
        body = renderBody(true, false);
        props = renderDesk();
        break;
      case "napping":
        body = renderBody(false, true);
        props = renderNap();
        break;
      default:
        body = renderBody(false, false);
    }

    return `<span class="atoll-logo is-${activity} is-phase-loop" aria-hidden="true">
      <svg class="atoll-logo-svg" viewBox="${VIEWBOX}" preserveAspectRatio="xMidYMid meet" shape-rendering="crispEdges">
        ${body}
        ${props}
      </svg>
    </span>`;
  }

  function startBlink(slot) {
    const logo = slot.querySelector(".atoll-logo");
    if (!logo || logo.classList.contains("is-napping")) return;

    let timer;
    const loop = () => {
      logo.classList.add("is-blinking");
      window.setTimeout(() => {
        logo.classList.remove("is-blinking");
        timer = window.setTimeout(loop, 2800 + Math.random() * 2800);
      }, 130);
    };
    timer = window.setTimeout(loop, 2000 + Math.random() * 1500);
  }

  function init() {
    document.querySelectorAll(".atoll-logo-slot[data-activity]").forEach((slot) => {
      const activity = slot.dataset.activity;
      slot.innerHTML = renderAtoll(activity);
      if (activity !== "napping") {
        startBlink(slot);
      }
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
