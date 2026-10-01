import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { JSDOM } from 'jsdom';

const html = readFileSync(new URL('../docs/index.html', import.meta.url), 'utf8');
const script = readFileSync(new URL('../docs/js/site.js', import.meta.url), 'utf8');
function setup(t, { reduced = false, observers = true } = {}) {
  const dom = new JSDOM(html, { url: 'https://example.test/Atoll/', runScripts: 'outside-only', pretendToBeVisual: true });
  t.after(() => dom.window.close());
  const { window } = dom;
  window.matchMedia = query => ({ matches: reduced && query.includes('reduced-motion'), addEventListener() {} });
  const timers = new Map();
  let counter = 0;
  window.setTimeout = (callback, delay) => { const id = ++counter; timers.set(id, { callback, delay }); return id; };
  window.clearTimeout = id => timers.delete(id);
  const observation = [];
  if (observers) window.IntersectionObserver = class {
    constructor(callback) { this.callback = callback; observation.push(this); }
    observe(element) { this.element = element; }
    unobserve() {}
  };
  let copied;
  Object.defineProperty(window.navigator, 'clipboard', { value: { writeText: async text => { copied = text; } } });
  window.eval(script);
  const $ = selector => window.document.querySelector(selector);
  const click = selector => $(selector).click();
  const flushTimers = () => {
    const entries = [...timers];
    timers.clear();
    entries.forEach(([, timer]) => timer.callback());
  };
  return { window, $, click, timers, flushTimers, observation, copied: () => copied };
}

test('approval decisions give feedback, collapse and replay', t => {
  const { $, click, flushTimers } = setup(t);
  assert.equal($('#demo-island').dataset.state, 'idle');
  for (const [decision, title] of [['approve', '已批准'], ['deny', '已拒绝'], ['always', '已永久放行']]) {
    click('[data-scene="approve"]');
    click(`[data-decision="${decision}"]`);
    assert.equal($('#result-title').textContent, title);
    assert.equal($('#demo-island').dataset.state, 'result');
    assert.equal($('[data-panel="approve"]').hidden, true);
    flushTimers();
    assert.equal($('#demo-island').dataset.state, 'idle');
    click('#demo-replay');
    assert.equal($('#demo-island').dataset.state, 'approve');
  }
});

test('rapid switching cancels stale collapse and resets plan/task contents', t => {
  const { $, click, timers, window } = setup(t);
  click('[data-scene="approve"]');
  click('[data-decision="deny"]');
  assert.equal(timers.size, 1);
  click('[data-scene="plan"]');
  assert.equal(timers.size, 0);
  $('input[value="卡片与排版"]').checked = true;
  $('#demo-plan-form').dispatchEvent(new window.Event('submit', { bubbles: true, cancelable: true }));
  assert.equal($('#demo-plan-title').textContent, '卡片与排版');
  assert.equal($('#demo-build').hidden, false);
  click('[data-scene="subagents"]');
  click('[data-task="shell"]');
  assert.match($('#task-transcript').textContent, /示例测试通过/);
  assert.equal($('#demo-task-list').hidden, true);
  click('#task-back');
  assert.equal($('#demo-task-list').hidden, false);
  click('[data-scene="plan"]');
  assert.equal($('#demo-build').hidden, true);
  assert.equal($('input[value="页面与动效"]').checked, true);
  assert.equal($('#task-transcript').textContent, '');
  for (const scene of ['approve', 'plan', 'subagents', 'approve']) click(`[data-scene="${scene}"]`);
  assert.equal($('#demo-island').dataset.state, 'approve');
  assert.equal(window.document.querySelectorAll('[data-panel]:not([hidden])').length, 1);
});

test('plan supports going back and approving construction', t => {
  const { $, click, window } = setup(t);
  const submit = () => $('#demo-plan-form').dispatchEvent(new window.Event('submit', { bubbles: true, cancelable: true }));
  click('[data-scene="plan"]');
  submit();
  assert.equal(window.document.activeElement.id, 'agree-build');
  click('#continue-plan');
  assert.equal($('#demo-plan-form').hidden, false);
  submit();
  click('#agree-build');
  assert.equal($('#result-title').textContent, '开始构建');
});

test('approval keyboard shortcuts stay inside the demo', t => {
  const { $, click, window } = setup(t);
  click('[data-scene="approve"]');
  window.document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Delete', bubbles: true }));
  assert.equal($('#demo-island').dataset.state, 'approve');
  $('[data-decision="approve"]').focus();
  $('[data-decision="approve"]').dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Enter', shiftKey: true, bubbles: true, cancelable: true }));
  assert.equal($('#result-title').textContent, '已永久放行');
});

test('install tabs, arrow navigation, copy and copy failure', async t => {
  const { $, click, window, copied } = setup(t);
  assert.equal($('#pane-windows').hidden, true);
  $('#tab-macos').dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true, cancelable: true }));
  assert.equal($('#pane-macos').hidden, true);
  assert.equal($('#tab-windows').getAttribute('aria-selected'), 'true');
  assert.equal(window.document.activeElement.id, 'tab-windows');
  click('#pane-windows .copy-btn');
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(copied(), $('#pane-windows .copy-btn').dataset.copy);
  assert.equal($('#pane-windows .copy-btn').textContent, '已复制');
  window.navigator.clipboard.writeText = async () => { throw new Error('denied'); };
  click('#pane-windows .copy-btn');
  await new Promise(resolve => setImmediate(resolve));
  assert.equal($('#pane-windows .copy-btn').textContent, '请手动复制');
});

test('mobile menu closes on Escape and restores focus', t => {
  const { $, click, window } = setup(t);
  click('#nav-toggle');
  assert.equal($('#nav-toggle').getAttribute('aria-expanded'), 'true');
  window.document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  assert.equal($('#nav-toggle').getAttribute('aria-expanded'), 'false');
  assert.equal(window.document.activeElement.id, 'nav-toggle');
});

test('reduced motion and missing observer do not block content or controls', t => {
  const { $, click } = setup(t, { reduced: true, observers: false });
  assert.ok($('#features h2').textContent.includes('工作向前'));
  assert.equal($('#features').hidden, false);
  click('[data-scene="subagents"]');
  assert.equal($('#demo-island').dataset.state, 'subagents');
  click('[data-task="explore"]');
  assert.match($('#task-transcript').textContent, /运行中/);
});

test('failed media receives readable alternative', t => {
  const { $, window } = setup(t);
  const img = $('.product-media img');
  img.dispatchEvent(new window.Event('error'));
  assert.equal(img.hidden, true);
  assert.equal($('.media-fallback').hidden, false);
  assert.match($('.media-fallback').textContent, /文件中转站/);
});

test('hidden page pauses motion and pending collapse until visibility returns', t => {
  const { $, click, window, timers, flushTimers } = setup(t);
  click('[data-scene="approve"]');
  click('[data-decision="approve"]');
  Object.defineProperty(window.document, 'hidden', { configurable: true, value: true });
  window.document.dispatchEvent(new window.Event('visibilitychange'));
  assert.equal(timers.size, 0);
  assert.equal(window.document.documentElement.classList.contains('page-hidden'), true);
  assert.equal($('#demo-island').dataset.state, 'result');
  Object.defineProperty(window.document, 'hidden', { configurable: true, value: false });
  window.document.dispatchEvent(new window.Event('visibilitychange'));
  assert.equal(timers.size, 1);
  flushTimers();
  assert.equal($('#demo-island').dataset.state, 'idle');
});

test('both installation instructions remain present without JavaScript', () => {
  const dom = new JSDOM(html);
  const $ = selector => dom.window.document.querySelector(selector);
  assert.equal($('#pane-macos').hasAttribute('hidden'), false);
  assert.equal($('#pane-windows').hasAttribute('hidden'), false);
  assert.match($('#pane-windows').textContent, /PowerShell/);
  for (const id of ['features', 'plan', 'subagents', 'agents', 'architecture', 'install', 'visual', 'updates']) assert.ok($('#' + id));
  dom.window.close();
});

test('mascot blink timers stop offscreen and on hidden pages', t => {
  const { $, window, observation, timers } = setup(t);
  window.eval(readFileSync(new URL('../docs/js/atoll-logo.js', import.meta.url), 'utf8'));
  if (window.document.readyState === 'loading') window.document.dispatchEvent(new window.Event('DOMContentLoaded'));
  const slot = $('.atoll-logo-slot[data-activity="idle"]');
  const observer = observation.find(item => item.element === slot);
  assert.ok(observer);
  observer.callback([{ isIntersecting: true }]);
  assert.equal(timers.size, 1);
  observer.callback([{ isIntersecting: false }]);
  assert.equal(timers.size, 0);
  observer.callback([{ isIntersecting: true }]);
  Object.defineProperty(window.document, 'hidden', { configurable: true, value: true });
  window.document.dispatchEvent(new window.Event('visibilitychange'));
  assert.equal(timers.size, 0);
  assert.equal(slot.querySelector('.atoll-logo').classList.contains('is-blinking'), false);
});
