/* Static website interactions. Demo decisions never leave the page. */
(() => {
  'use strict';
  const $ = (selector, root = document) => root.querySelector(selector);
  const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];
  document.documentElement.classList.add('js-ready');
  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');

  // Navigation and keyboard-accessible mobile menu.
  const nav = $('#nav');
  const navToggle = $('#nav-toggle');
  const navLinks = $('#nav-links');
  function closeMenu(returnFocus = false) {
    navLinks.classList.remove('open');
    navToggle.setAttribute('aria-expanded', 'false');
    navToggle.setAttribute('aria-label', '打开菜单');
    if (returnFocus) navToggle.focus();
  }
  function updateNav() { nav.classList.toggle('scrolled', window.scrollY > 16); }
  updateNav();
  window.addEventListener('scroll', updateNav, { passive: true });
  navToggle.addEventListener('click', () => {
    const open = navLinks.classList.toggle('open');
    navToggle.setAttribute('aria-expanded', String(open));
    navToggle.setAttribute('aria-label', open ? '关闭菜单' : '打开菜单');
  });
  $$('a', navLinks).forEach(link => link.addEventListener('click', () => closeMenu()));
  document.addEventListener('keydown', event => {
    if (event.key === 'Escape' && navLinks.classList.contains('open')) closeMenu(true);
  });
  document.addEventListener('click', event => {
    if (!nav.contains(event.target)) closeMenu();
  });
  window.matchMedia('(min-width: 641px)').addEventListener('change', event => {
    if (event.matches) closeMenu();
  });

  // Tabs expose both platforms if scripts are unavailable.
  const installTabs = $$('.install-tab');
  function selectInstall(tab, focus = false) {
    installTabs.forEach(item => {
      const active = item === tab;
      item.classList.toggle('active', active);
      item.setAttribute('aria-selected', String(active));
      item.tabIndex = active ? 0 : -1;
      const panel = document.getElementById(item.getAttribute('aria-controls'));
      panel.hidden = !active;
      panel.classList.toggle('active', active);
    });
    if (focus) tab.focus();
  }
  installTabs.forEach((tab, index) => {
    tab.addEventListener('click', () => selectInstall(tab));
    tab.addEventListener('keydown', event => {
      let next;
      if (event.key === 'ArrowRight') next = (index + 1) % installTabs.length;
      if (event.key === 'ArrowLeft') next = (index + installTabs.length - 1) % installTabs.length;
      if (event.key === 'Home') next = 0;
      if (event.key === 'End') next = installTabs.length - 1;
      if (next !== undefined) {
        event.preventDefault();
        selectInstall(installTabs[next], true);
      }
    });
  });
  selectInstall(installTabs[0]);
  $$('.copy-btn').forEach(button => {
    let feedbackTimer;
    button.setAttribute('aria-live', 'polite');
    button.addEventListener('click', async () => {
      window.clearTimeout(feedbackTimer);
      try {
        await navigator.clipboard.writeText(button.dataset.copy);
        button.textContent = '已复制';
        button.classList.add('copied');
      } catch {
        button.textContent = '请手动复制';
        button.classList.remove('copied');
      }
      feedbackTimer = window.setTimeout(() => {
        button.textContent = '复制';
        button.classList.remove('copied');
      }, 2200);
    });
  });

  // One state owner and one cancellable timer keep rapid scene changes stable.
  const island = $('#demo-island');
  const sceneButtons = $$('[data-scene]');
  const panels = $$('[data-panel]', island);
  const caption = $('#demo-caption');
  const status = $('#demo-status');
  const terminal = $('#terminal-status');
  let scene = 'approve';
  let state = 'idle';
  let settleTimer;
  let settleDue = 0;
  let settleRemaining = 0;
  const scenes = {
    approve: { label: '等待审批', caption: '点击 Approve、Deny 或 Always，决定 Agent 的下一步。', terminal: '等待权限：npm test --run' },
    plan: { label: '规划问答', caption: '选择优化范围，提交回答，再确认是否开始构建。', terminal: '等待你确认规划与构建范围。' },
    subagents: { label: '后台任务', caption: '点击一个任务，看看它的执行状态与详情。', terminal: '2 个子任务正在协作，状态同步到浮岛。' },
  };
  function cancelSettle() {
    window.clearTimeout(settleTimer);
    settleTimer = undefined;
    settleDue = 0;
    settleRemaining = 0;
  }
  function setState(next, message) {
    state = next;
    island.dataset.state = next;
    panels.forEach(panel => {
      // If a hidden subtree held focus, move to its scene control first.
      if (panel.dataset.panel !== next && panel.contains(document.activeElement)) {
        sceneButtons.find(button => button.dataset.scene === scene)?.focus({ preventScroll: true });
      }
      panel.hidden = panel.dataset.panel !== next;
    });
    $('#demo-idle-note').hidden = next !== 'idle';
    $('#island-label').textContent = next === 'idle' ? '在线监听' : next === 'result' ? '决定已送达' : scenes[scene].label;
    $('#island-count').textContent = next === 'approve' ? '1' : next === 'subagents' ? '2' : '0';
    if (message) status.textContent = message;
  }
  function resetPanels() {
    $('#demo-plan-form').reset();
    $('#demo-plan-form').hidden = false;
    $('#demo-build').hidden = true;
    $('#demo-task-list').hidden = false;
    $('#demo-task-detail').hidden = true;
    $('#task-transcript').textContent = '';
  }
  function launchScene(next, focus = false) {
    if (!Object.hasOwn(scenes, next)) return;
    cancelSettle();
    scene = next;
    resetPanels();
    sceneButtons.forEach(button => button.setAttribute('aria-pressed', String(button.dataset.scene === scene)));
    setState(scene, scenes[scene].caption);
    caption.textContent = scenes[scene].caption;
    terminal.textContent = scenes[scene].terminal;
    if (focus) $('[data-panel="' + scene + '"] button, [data-panel="' + scene + '"] input', island)?.focus({ preventScroll: true });
  }
  function scheduleSettle(delay) {
    settleRemaining = delay;
    if (document.hidden) return;
    settleDue = Date.now() + delay;
    settleTimer = window.setTimeout(() => {
      settleTimer = undefined;
      settleRemaining = 0;
      settleDue = 0;
      // Keep the result readable while someone is using its controls.
      setState('idle', '浮岛已收回，Agent 继续执行。可选择场景再次体验。');
      caption.textContent = '浮岛自动收回。你继续专注，下一次需要你时再出现。';
    }, delay);
  }
  function finish(title, description, terminalMessage, denied = false) {
    cancelSettle();
    $('#result-title').textContent = title;
    $('#result-description').textContent = description;
    $('#result-mark').textContent = denied ? '×' : '✓';
    terminal.textContent = terminalMessage;
    setState('result', title + '。' + description);
    caption.textContent = title + ' · 稍后自动收回，也可点击重播再次体验。';
    scheduleSettle(2800);
  }
  sceneButtons.forEach(button => button.addEventListener('click', () => launchScene(button.dataset.scene)));
  $('#demo-replay').addEventListener('click', () => launchScene(scene));
  $$('[data-launch]').forEach(link => link.addEventListener('click', () => launchScene(link.dataset.launch, true)));
  $$('[data-decision]', island).forEach(button => button.addEventListener('click', () => {
    if (state !== 'approve') return;
    const decision = button.dataset.decision;
    if (decision === 'deny') finish('已拒绝', 'Agent 收到拒绝，当前命令不会执行。', '你已拒绝本次测试命令。', true);
    else if (decision === 'always') finish('已永久放行', '演示规则已保存，下次相同请求自动通过。', '演示规则已生效，Agent 继续运行测试。');
    else finish('已批准', '决定已送回 Agent，继续你的工作。', '权限已批准，正在运行项目测试。');
  }));
  // Shortcuts only apply inside the approval demo, never to the entire page.
  island.addEventListener('keydown', event => {
    if (state !== 'approve' || event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.key !== 'Enter' && event.key !== 'Delete') return;
    // Preserve native Enter activation when a decision button is focused.
    if (event.key === 'Enter' && !event.shiftKey && event.target.matches('button')) return;
    event.preventDefault();
    const decision = event.key === 'Delete' ? 'deny' : event.shiftKey ? 'always' : 'approve';
    $('[data-decision="' + decision + '"]', island).click();
  });
  $('#demo-plan-form').addEventListener('submit', event => {
    event.preventDefault();
    if (state !== 'plan') return;
    $('#demo-plan-title').textContent = new FormData(event.currentTarget).get('demo-scope');
    $('#demo-build').hidden = false;
    $('#demo-plan-form').hidden = true;
    $('#agree-build').focus({ preventScroll: true });
    status.textContent = '计划已就绪。可以继续规划，或同意构建。';
    caption.textContent = '计划已就绪。由你确认，Agent 才开始构建。';
  });
  $('#continue-plan').addEventListener('click', () => {
    if (state !== 'plan') return;
    $('#demo-plan-form').hidden = false;
    $('#demo-build').hidden = true;
    $('#demo-plan-form input:checked').focus({ preventScroll: true });
    status.textContent = '继续规划，请调整优化范围。';
    caption.textContent = scenes.plan.caption;
  });
  $('#agree-build').addEventListener('click', () => {
    if (state === 'plan') finish('开始构建', '计划已确认，Agent 按约定的范围开始工作。', '计划已批准，开始执行构建。');
  });
  const tasks = {
    explore: '运行中 · 00:24\n› 阅读页面结构\n› 检查卡片与响应式布局\n› 正在整理优化建议…',
    shell: '已完成 · 00:12\n› 运行 npm test --run\n✓ 示例测试通过\n✓ 结果已回传主 Agent',
  };
  $$('[data-task]', island).forEach(button => button.addEventListener('click', () => {
    if (state !== 'subagents') return;
    $('#task-title').textContent = button.dataset.task;
    $('#task-transcript').textContent = tasks[button.dataset.task];
    $('#demo-task-detail').hidden = false;
    $('#demo-task-list').hidden = true;
    $('#task-back').focus({ preventScroll: true });
    status.textContent = '正在查看 ' + button.dataset.task + ' 的示例执行详情。';
  }));
  $('#task-back').addEventListener('click', () => {
    $('#demo-task-list').hidden = false;
    $('#demo-task-detail').hidden = true;
    $('[data-task="' + $('#task-title').textContent + '"]', island).focus({ preventScroll: true });
  });

  // Images have a readable replacement instead of broken icon / empty space.
  $$('.product-media img').forEach(img => {
    function showFallback() {
      img.hidden = true;
      const fallback = $('.media-fallback', img.parentElement);
      if (fallback) fallback.hidden = false;
    }
    img.addEventListener('error', showFallback);
    if (img.complete && img.naturalWidth === 0) showFallback();
  });

  // CSS animation zones are paused offscreen. Reveal is additive, never hides text.
  const motionZones = $$('[data-motion-zone], .closing-cta');
  if ('IntersectionObserver' in window) {
    const revealObserver = new IntersectionObserver(entries => {
      entries.forEach(entry => {
        if (!entry.isIntersecting) return;
        if (!reducedMotion.matches) {
          entry.target.classList.add('is-entering');
          entry.target.addEventListener('animationend', () => entry.target.classList.remove('is-entering'), { once: true });
        }
        revealObserver.unobserve(entry.target);
      });
    }, { threshold: 0, rootMargin: '0px 0px -20px 0px' });
    $$('.reveal').forEach(element => revealObserver.observe(element));
    const motionObserver = new IntersectionObserver(entries => {
      entries.forEach(entry => entry.target.classList.toggle('motion-paused', !entry.isIntersecting));
    }, { threshold: 0 });
    motionZones.forEach(element => {
      element.classList.add('motion-paused');
      motionObserver.observe(element);
    });
  }
  function updatePageVisibility() {
    document.documentElement.classList.toggle('page-hidden', document.hidden);
    if (state !== 'result') return;
    if (document.hidden && settleTimer !== undefined) {
      settleRemaining = Math.max(0, settleDue - Date.now());
      window.clearTimeout(settleTimer);
      settleTimer = undefined;
    } else if (!document.hidden && settleTimer === undefined && settleRemaining > 0) scheduleSettle(settleRemaining);
  }
  document.addEventListener('visibilitychange', updatePageVisibility);
  updatePageVisibility();
})();
