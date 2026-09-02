/* Atoll logo renderer for landing page.
 * 与 src/AtollLogo.tsx 手工同步的简化镜像（loop-only）。
 * 同步清单：无腿（腿仅在动作需要时出现）、eye 变体、
 * coding / thinking / napping 多段式循环、fishing / stargazing / garden / music / gaming 彩蛋。 */
(function () {
  const VIEWBOX = "-16 -36 96 108";
  const BODY = "#38BDD8";
  const TOP = "#5FD8EC";
  const EYE = "#1a1a1a";

  function renderBody(eyeVariant) {
    let eyes;
    if (eyeVariant === "closed") {
      eyes = `<rect class="atoll-eye atoll-eye-left" x="18" y="28" width="6" height="2.5" fill="${EYE}"/>
              <rect class="atoll-eye atoll-eye-right" x="38" y="28" width="6" height="2.5" fill="${EYE}"/>`;
    } else if (eyeVariant === "happy") {
      eyes = `<rect class="atoll-eye atoll-eye-left" x="17" y="29" width="9" height="2.5" fill="${EYE}"/>
              <rect class="atoll-eye atoll-eye-right" x="36" y="29" width="9" height="2.5" fill="${EYE}"/>`;
    } else {
      eyes = `<rect class="atoll-eye atoll-eye-left" x="19" y="24" width="5" height="11" fill="${EYE}"/>
              <rect class="atoll-eye atoll-eye-right" x="38" y="24" width="5" height="11" fill="${EYE}"/>`;
    }
    return `<g class="atoll-body-group">
      <rect x="8" y="18" width="48" height="26" fill="${BODY}"/>
      <rect x="8" y="18" width="48" height="5" fill="${TOP}"/>
      ${eyes}
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
      <text class="atoll-code-tag" x="34" y="55" font-size="7" font-family="ui-monospace,monospace" font-weight="700" fill="#6BE088">&lt;/&gt;</text>
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
      <text class="atoll-think-mark atoll-think-q" x="33" y="-13" font-size="18" font-weight="800" fill="#7C6BC4" font-family="ui-monospace,monospace">?</text>
      <text class="atoll-think-mark atoll-think-ex" x="33" y="-13" font-size="19" font-weight="800" fill="#5FBF7A" font-family="ui-monospace,monospace">!</text>
      <g fill="#7C6BC4">
        <circle class="atoll-dot atoll-dot-0" cx="44" cy="-17" r="2.5"/>
        <circle class="atoll-dot atoll-dot-1" cx="50" cy="-17" r="2.5"/>
        <circle class="atoll-dot atoll-dot-2" cx="56" cy="-17" r="2.5"/>
      </g>
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

  function renderCoffee() {
    return `<g class="atoll-prop atoll-coffee-stand">
      <rect x="50" y="54" width="16" height="3" fill="#8B7355"/>
      <rect x="52" y="57" width="2" height="6" fill="#6B5344"/>
      <rect x="62" y="57" width="2" height="6" fill="#6B5344"/>
      <g class="atoll-mug-group">
        <rect x="52" y="38" width="12" height="16" fill="#F5F0E8"/>
        <rect x="54" y="40" width="8" height="11" fill="#6B4226"/>
        <rect x="64" y="42" width="4" height="3" fill="#F5F0E8"/>
        <rect class="atoll-steam atoll-steam-0" x="54" y="30" width="3" height="6" rx="1" fill="#fff" fill-opacity="0.7"/>
        <rect class="atoll-steam atoll-steam-1" x="60" y="28" width="3" height="7" rx="1" fill="#fff" fill-opacity="0.55"/>
      </g>
      <rect class="atoll-coffee-grip" x="48" y="42" width="3" height="5" fill="#2A8FA8" opacity="0.7"/>
    </g>`;
  }

  function renderFishing() {
    return `<g class="atoll-prop atoll-water">
        <rect x="44" y="60" width="36" height="5" fill="#1F5F7A"/>
        <rect x="44" y="63" width="36" height="3" fill="#2A7D9E"/>
        <ellipse class="atoll-ripple" cx="72" cy="62" rx="7" ry="2" fill="none" stroke="#7FD8F0" stroke-width="1"/>
        <ellipse class="atoll-ripple r2" cx="72" cy="62" rx="7" ry="2" fill="none" stroke="#7FD8F0" stroke-width="1"/>
      </g>
      <g transform="translate(63,57)"><g class="atoll-fish"><g class="atoll-fish-inner">
        <rect x="0" y="0" width="9" height="5" fill="#FF8A50"/>
        <polygon points="9,0 14,2.5 9,5" fill="#FF8A50"/>
        <rect x="2" y="1" width="1.6" height="1.6" fill="#1a1a1a"/>
      </g></g></g>
      <text class="atoll-exclaim" x="21" y="-6" font-size="19" font-weight="800" fill="#FFD24A" font-family="ui-monospace,monospace">!</text>
      <g class="atoll-rod">
        <line x1="50" y1="24" x2="72" y2="-2" stroke="#8B5A2B" stroke-width="2.5" stroke-linecap="round"/>
        <rect x="47" y="21" width="4" height="5" rx="1" fill="#5F707C"/>
      </g>
      <g class="atoll-line">
        <line x1="72" y1="-1" x2="72" y2="51" stroke="#CFD8DD" stroke-width="1"/>
        <g class="atoll-bobber">
          <rect x="70" y="51" width="4" height="2" fill="#E05555"/>
          <rect x="70" y="53" width="4" height="2" fill="#F5F0E8"/>
        </g>
      </g>
      <g class="atoll-splash">
        <rect x="63" y="52" width="2" height="2" fill="#7FD8F0"/>
        <rect x="68" y="51" width="2" height="2" fill="#7FD8F0"/>
        <rect x="73" y="52" width="2" height="2" fill="#7FD8F0"/>
      </g>`;
  }

  function renderStargazing() {
    const star = (x, y, s, cls) =>
      `<g transform="translate(${x},${y}) scale(${s})"><path class="atoll-star ${cls}" d="M0 -3 L0.9 -0.9 L3 0 L0.9 0.9 L0 3 L-0.9 0.9 L-3 0 L-0.9 -0.9 Z" fill="#F4E9C8"/></g>`;
    return `<g class="atoll-prop atoll-sky">
      <defs>
        <linearGradient id="atoll-meteor-grad" x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" stop-color="#FFF7D6" stop-opacity="0"/>
          <stop offset="1" stop-color="#FFF7D6" stop-opacity="0.9"/>
        </linearGradient>
      </defs>
      <circle class="atoll-moon-glow" cx="-4" cy="-22" r="11.5" fill="#F0E6C8"/>
      <circle cx="-4" cy="-22" r="7.5" fill="#F0E6C8"/>
      <circle cx="-6.5" cy="-24" r="1.6" fill="#D9CBA4"/>
      <circle cx="-1" cy="-20" r="1.1" fill="#D9CBA4"/>
      ${star(10, -28, 1, "s1")}
      ${star(34, -31, 0.85, "s2")}
      ${star(58, -24, 0.7, "s3")}
      ${star(68, -6, 0.6, "s4")}
      <g transform="translate(66,-28)"><g class="atoll-meteor">
        <rect x="2" y="-1" width="15" height="2" fill="url(#atoll-meteor-grad)"/>
        <circle cx="0" cy="0" r="2.2" fill="#FFF7D6"/>
      </g></g>
    </g>`;
  }

  function renderGarden() {
    return `<g class="atoll-plant">
        <rect x="29" y="58" width="15" height="4" rx="1" fill="#6B4226"/>
        <g class="atoll-sprout"><rect x="35.3" y="49" width="2.4" height="9" fill="#4CAF50"/></g>
        <g transform="rotate(-24 35.5 51)"><rect class="atoll-leaf atoll-leaf-l" x="29.5" y="50.8" width="5.5" height="2.4" rx="1.2" fill="#66BB6A"/></g>
        <g transform="rotate(22 37.5 50)"><rect class="atoll-leaf atoll-leaf-r" x="37.5" y="48.5" width="5.5" height="2.4" rx="1.2" fill="#66BB6A"/></g>
      </g>
      <g class="atoll-can">
        <rect x="54" y="35" width="12" height="10" rx="1" fill="#7E8E9C"/>
        <rect x="56" y="32.5" width="8" height="2.5" rx="1.2" fill="#66757F"/>
        <line x1="54.5" y1="38" x2="45.5" y2="42" stroke="#66757F" stroke-width="2.5" stroke-linecap="round"/>
        <rect x="55" y="43" width="3" height="4.5" fill="#2A8FA8" opacity="0.7"/>
      </g>
      <g class="atoll-drops">
        <g class="atoll-drop"><rect x="44.6" y="49" width="2.4" height="3.2" rx="1.2" fill="#8FD8F0"/></g>
        <g class="atoll-drop d2"><rect x="45.6" y="50" width="2.4" height="3.2" rx="1.2" fill="#8FD8F0"/></g>
        <g class="atoll-drop d3"><rect x="43.8" y="50.5" width="2.4" height="3.2" rx="1.2" fill="#8FD8F0"/></g>
      </g>`;
  }

  function renderMusic() {
    return `<text class="atoll-note n1" x="-2" y="4" font-size="13" fill="#7FD8F0" font-family="ui-monospace,monospace">&#9834;</text>
      <text class="atoll-note n2" x="60" y="-2" font-size="15" fill="#FFD24A" font-family="ui-monospace,monospace">&#9835;</text>
      <text class="atoll-note n3" x="66" y="16" font-size="11" fill="#F49AC1" font-family="ui-monospace,monospace">&#9834;</text>
      <text class="atoll-note n4" x="-12" y="14" font-size="12" fill="#9BF0C0" font-family="ui-monospace,monospace">&#9835;</text>
      <g class="atoll-body-outer"><g class="atoll-body-mid"></g></g>
      <text class="atoll-note atoll-note-accent" x="26" y="-16" font-size="18" font-weight="700" fill="#7FD8F0" font-family="ui-monospace,monospace">&#9834;</text>
      <g class="atoll-phones">
        <rect x="9" y="14" width="46" height="4" rx="2" fill="#16161C"/>
        <rect x="5" y="19" width="7" height="15" rx="2.5" fill="#16161C"/>
        <rect x="52" y="19" width="7" height="15" rx="2.5" fill="#16161C"/>
        <rect x="7" y="21.5" width="3" height="10" rx="1.5" fill="#3E4A58"/>
        <rect x="54" y="21.5" width="3" height="10" rx="1.5" fill="#3E4A58"/>
      </g>`;
  }

  function renderGaming() {
    return `<g class="atoll-console">
        <rect x="16" y="40" width="36" height="17" rx="2.5" fill="#2A2A3A"/>
        <rect x="19" y="43" width="17" height="11" fill="#0D1117"/>
        <rect class="atoll-ship" x="21" y="46" width="3" height="3" fill="#6BE088"/>
        <rect class="atoll-enemy atoll-enemy-a" x="30.5" y="44.5" width="3" height="3" fill="#E06060"/>
        <rect class="atoll-enemy atoll-enemy-b" x="31.5" y="49" width="3" height="3" fill="#E06060"/>
        <rect class="atoll-laser" x="24.5" y="46.8" width="6" height="1.4" fill="#FFD24A" opacity="0"/>
        <text class="atoll-win" x="20.5" y="52" font-size="6" font-weight="800" fill="#6BE088" font-family="ui-monospace,monospace">WIN</text>
        <rect class="atoll-flash" x="19" y="43" width="17" height="11" fill="#FFFFFF" opacity="0"/>
        <rect x="38.5" y="46.5" width="7" height="2.2" fill="#555566"/>
        <rect x="41" y="44" width="2.2" height="7" fill="#555566"/>
        <circle cx="48.5" cy="46" r="2" fill="#E05555"/>
        <circle cx="49.5" cy="51" r="2" fill="#5599E0"/>
      </g>
      <g class="atoll-thumbs">
        <g class="atoll-thumb"><rect x="21" y="56" width="4.5" height="3.5" fill="#2A8FA8" opacity="0.8"/></g>
        <g class="atoll-thumb t2"><rect x="41" y="56" width="4.5" height="3.5" fill="#2A8FA8" opacity="0.8"/></g>
      </g>
      <g class="atoll-confetti">
        <rect x="33" y="34" width="3" height="3" fill="#FFD24A"/>
        <rect x="37" y="34" width="3" height="3" fill="#9BF0C0"/>
        <rect x="41" y="34" width="3" height="3" fill="#F49AC1"/>
        <rect x="35" y="38" width="3" height="3" fill="#68B8F8"/>
        <rect x="39" y="38" width="3" height="3" fill="#FF8A50"/>
      </g>`;
  }

  const ACTIVITIES = {
    idle: { body: "normal", props: null },
    thinking: { body: "normal", props: renderThought },
    coding: { body: "normal", props: renderDesk },
    napping: { body: "closed", props: renderNap },
    fishing: { body: "normal", props: renderFishing },
    stargazing: { body: "normal", props: renderStargazing },
    garden: { body: "normal", props: renderGarden },
    coffee: { body: "normal", props: renderCoffee },
    music: { body: "happy", props: renderMusic },
    gaming: { body: "normal", props: renderGaming },
  };

  function renderAtoll(activity) {
    const config = ACTIVITIES[activity] ?? ACTIVITIES.idle;
    const body = renderBody(config.body);
    let props = "";
    if (activity === "music") {
      // music 的 props 分前后两层，body 夹在中间的 outer/mid 包装层里
      const notes = config.props();
      const wrapper = '<g class="atoll-body-outer"><g class="atoll-body-mid"></g></g>';
      props = notes.replace(
        wrapper,
        `<g class="atoll-body-outer"><g class="atoll-body-mid">${body}</g></g>`,
      );
      return `<span class="atoll-logo is-${activity} is-phase-loop" aria-hidden="true">
        <svg class="atoll-logo-svg" viewBox="${VIEWBOX}" preserveAspectRatio="xMidYMid meet" shape-rendering="crispEdges">${props}</svg>
      </span>`;
    }
    if (config.props) props = config.props();
    return `<span class="atoll-logo is-${activity} is-phase-loop" aria-hidden="true">
      <svg class="atoll-logo-svg" viewBox="${VIEWBOX}" preserveAspectRatio="xMidYMid meet" shape-rendering="crispEdges">
        ${body}
        ${props}
      </svg>
    </span>`;
  }

  function startBlink(slot) {
    const logo = slot.querySelector(".atoll-logo");
    if (!logo) return;
    // 眯眼/闭眼姿态（napping/music）不做眨眼
    if (logo.classList.contains("is-napping") || logo.classList.contains("is-music")) return;

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
      startBlink(slot);
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
