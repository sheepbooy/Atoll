/** Run against npm run dev: checks real CSS/SVG timelines, beyond jsdom. */
import assert from "node:assert/strict";
import { chromium } from "playwright";

const browser = await chromium.launch({ headless: true, channel: process.env.PLAYWRIGHT_CHANNEL || "chrome" });
try {
  const page = await browser.newPage({ viewport: { width: 1200, height: 1000 } });
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(`${process.env.ATOLL_PREVIEW_URL || "http://127.0.0.1:1420"}/?preview=agents`);
  await page.waitForSelector(".agent-playground-card");
  assert.equal(await page.locator(".agent-playground-card").count(), 6);
  const working = await page.evaluate(() => [...document.querySelectorAll(".agent-playground-stage .agent-mascot")].map(root => ({
    agent: root.closest("[data-agent]").dataset.agent,
    timelines: root.getAnimations({ subtree: true }).map(animation => animation.animationName).filter(name => name?.startsWith("agent-work-")),
  })));
  for (const character of working) assert.ok(character.timelines.length > 0, `${character.agent}: missing work parts`);
  assert.equal(new Set(working.map(character => character.timelines.sort().join(","))).size, 6);

  await page.evaluate(() => { window.originalAgentSVGs = [...document.querySelectorAll(".agent-playground-stage svg")]; });
  for (const label of ["开始工作", "提醒", "通过", "拒绝", "回合结束", "加入", "离开"]) {
    await page.getByRole("button", { name: label, exact: true }).click();
    await page.waitForTimeout(80);
    assert.equal(await page.locator(".agent-playground-stage .has-action").count(), 6, label);
    const shots = await page.evaluate(() => [...document.querySelectorAll(".agent-playground-stage .agent-mascot")].map(root =>
      root.getAnimations({ subtree: true }).filter(animation => animation.animationName?.startsWith("agent-shot-")).length));
    assert.ok(shots.every(count => count > 0), `${label}: missing part feedback`);
    assert.ok(await page.evaluate(() => window.originalAgentSVGs.every((svg, i) => svg === document.querySelectorAll(".agent-playground-stage svg")[i])));
    const duration = await page.evaluate(() => Math.max(...[...document.querySelectorAll('.agent-playground-stage .agent-mascot')].map(root => parseFloat(root.style.getPropertyValue('--agent-shot-ms')))));
    await page.waitForTimeout(duration + 100);
    assert.equal(await page.locator(".has-action").count(), 0, `${label}: lingering feedback`);
  }

  // All seven moods, plus work, must remain usable at every preview size.
  for (let state = 0; state < 8; state++) {
    await page.getByLabel("状态", { exact: true }).selectOption(String(state));
    await page.waitForTimeout(1800);
    assert.equal(await page.locator(".has-action").count(), 0, `state ${state}: lingering entrance`);
    assert.equal(await page.locator(".agent-playground-sizes .agent-mascot").count(), 24);
  }
  await page.getByLabel("状态", { exact: true }).selectOption("1");
  await page.getByLabel("省电", { exact: true }).check();
  assert.equal(await page.locator(".is-loop-working").count(), 0);
  await page.getByRole("button", { name: "通过", exact: true }).click();
  await page.waitForTimeout(80);
  assert.equal(await page.locator(".agent-playground-stage .has-action").count(), 6);
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.waitForTimeout(80);
  assert.equal(await page.locator(".has-action").count(), 0);
  assert.equal(await page.locator(".is-motion-disabled").count(), 30);
  const resting = await page.evaluate(() => [...document.querySelectorAll(".agent-mascot [data-part]")].every(part => getComputedStyle(part).transform === "none"));
  assert.ok(resting, "reduced motion must restore the parts");
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.waitForTimeout(80);
  assert.equal(await page.locator(".has-action").count(), 0, "expired feedback replayed after recovery");

  // Saving must not freeze asynchronous UI content at its transparent entrance.
  await page.evaluate(() => {
    const island = document.createElement("div");
    island.className = "island is-expanded is-energy-saving";
    for (const className of ["session-item", "stagger-child", "island-panel-footer", "approval-view is-arrive"]) {
      const node = document.createElement("div"); node.className = className; island.append(node);
    }
    island.id = "energy-visibility-probe"; document.body.append(island);
  });
  await page.waitForTimeout(800);
  assert.ok(await page.evaluate(() => [...document.querySelector("#energy-visibility-probe").children].every(node => getComputedStyle(node).opacity === "1")));

  // Compare actual poses, rather than merely checking for distinct keyframe names.
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.goto(`${process.env.ATOLL_PREVIEW_URL || "http://127.0.0.1:1420"}/?preview=agents&view=states`);
  await page.waitForSelector('.agent-comparison-row');
  assert.equal(await page.locator('.agent-comparison-cell').count(),24);
  await page.getByRole('button',{name:'重放所有反馈',exact:true}).click();
  const poses = await page.evaluate(() => [...document.querySelectorAll('.agent-comparison-row')].filter(row => row.dataset.agent !== 'claude').map(row => {
    const cells = [...row.querySelectorAll('.agent-comparison-stage .agent-mascot')];
    return {agent:row.dataset.agent,poses:cells.map(root => {
      for (const animation of root.getAnimations({subtree:true})) {
        animation.pause(); animation.currentTime = Number(animation.effect.getTiming().duration) * .35;
      }
      const svg = root.querySelector('svg'), contour = root.querySelector('animate');
      if (contour) { svg.pauseAnimations(); svg.setCurrentTime(contour.getStartTime() + Number(contour.getAttribute('dur').replace('ms','')) * .00035); }
      return [...root.querySelectorAll('[data-part]')].map(part => {
        const bounds=part.getBoundingClientRect();
        return [part.dataset.part,getComputedStyle(part).transform,Number(bounds.width.toFixed(2)),Number(bounds.height.toFixed(2))];
      });
    })};
  }));
  for (const row of poses) assert.equal(new Set(row.poses.map(pose=>JSON.stringify(pose))).size,4,`${row.agent}: indistinguishable state poses`);
  await page.getByRole('button',{name:'暂停并复位',exact:true}).click();
  assert.equal(await page.locator('.agent-comparison-cell animate').count(),0,'pause must remove native contour animation');
  const smallSizes = await page.evaluate(() => [...document.querySelectorAll('.agent-comparison-row:not([data-agent="claude"]) .agent-mascot.is-small')].map(root => {
    const size = parseFloat(root.style.height), bounds=root.querySelector('[data-part="body"]').getBoundingClientRect();
    return {size,height:bounds.height};
  }));
  assert.ok(smallSizes.every(({size,height})=>height >= size*.85 && height <= size*1.05),'small marks must occupy the actual icon size');
  assert.deepEqual(errors, []);
  console.log("Agent motion browser checks passed: distinct state poses, actual small sizes, stable SVGs, energy visibility and reduced motion.");
} finally {
  await browser.close();
}
