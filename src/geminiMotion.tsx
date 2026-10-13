import { useLayoutEffect, useMemo, useRef } from "react";
import type { MascotTimeline } from "./mascotMotion";
import { GEMINI_OUTLINE } from "./geminiOutline";

type Tips = { top?: number; right?: number; bottom?: number; left?: number; rightWave?: number; leftWave?: number };
type Frame = readonly [number, Tips];
const REST: Tips = {};
const WORK: readonly Frame[] = [[0,REST],[180,REST],
  [430,{right:3,rightWave:-5,leftWave:2}],[680,{right:1,rightWave:-1}],
  [1030,{left:3,leftWave:-5,rightWave:2}],[1290,{left:1,leftWave:-1}],
  [1580,{right:2.5,rightWave:-4}],[1830,{left:2.5,leftWave:-4}],[2160,REST],[2500,REST],[3200,REST]];
const ATTENTION: readonly Frame[] = [[0,REST],[140,REST],
  [330,{top:3.6,right:2.4,rightWave:-5.5}],[460,{top:3.6,right:2.8,rightWave:1.6}],
  [590,{top:3.6,right:2.4,rightWave:-5.5}],[720,{top:3.6,right:2.8,rightWave:1.6}],
  [850,{top:3.6,right:2.4,rightWave:-4.2}],[1200,{top:3.6,right:2.4,rightWave:-4.2}],[1540,REST]];
const APPROVED: readonly Frame[] = [[0,REST],[150,{top:-1,bottom:-1}],
  [310,{top:2,right:2.7,bottom:2,left:2.7}],[450,{top:3,right:3,bottom:3,left:3}],
  [650,{right:1,left:1}],[790,REST],[950,REST]];
const DENIED: readonly Frame[] = [[0,REST],[130,REST],[250,{right:-2,left:-2,top:1}],
  [380,{right:-2,left:-2,top:1}],[510,{right:-2.3,left:-2.3}],[640,{right:-2.3,left:-2.3}],
  [780,{right:-2.5,left:-2.5,top:-1}],[1010,REST]];
const OTHER: Record<string, readonly Frame[]> = {
  idle:[[0,REST],[8400,REST],[9600,{top:-1,bottom:-1}],[10800,{left:1.5,right:1.5}],[12000,REST]],
  concerned:[[0,REST],[180,{left:2,right:-1}],[420,{left:-1,right:2}],[650,{left:1.5,right:-.5}],[900,REST]],
  downcast:[[0,REST],[280,{top:-3,left:-1.5,leftWave:2}],[660,{top:-3,left:-1.5,leftWave:2}],[900,REST]],
  turnEnded:[[0,REST],[180,{top:-2}],[360,{right:-2}],[540,{bottom:-2}],[700,{left:-2}],[900,REST]],
  join:[[0,REST],[250,{top:2,right:1}],[550,{right:2,left:2}],[900,REST]],
  leave:[[0,REST],[350,{left:-2,right:-2}],[650,{top:-2,bottom:-2}],[900,REST]],
  sleeping:[[0,REST],[380,{top:-3,bottom:-3}],[660,{top:-2,bottom:-2,left:-2,right:-2}],[900,REST]],
};

/** One connected contour replaces independently scaled slices, so waving tips cannot leave gaps. */
export function geminiGesturePath(tips: Tips) {
  return GEMINI_OUTLINE.map(([x,y],index) => {
    const weight = (distance: number) => Math.pow(Math.max(0,(distance-1)/11),1.8);
    const top = weight(-y), right = weight(x), bottom = weight(y), left = weight(-x);
    const dx = x + right * (tips.right ?? 0) - left * (tips.left ?? 0) + 12;
    const dy = y - top * (tips.top ?? 0) + bottom * (tips.bottom ?? 0) + right * (tips.rightWave ?? 0) + left * (tips.leftWave ?? 0) + 12;
    return `${index ? "L" : "M"}${dx.toFixed(3)} ${dy.toFixed(3)}`;
  }).join("") + "Z";
}

function gestureFrames(timeline: MascotTimeline) {
  const state = timeline.state;
  if (state === "working" || state === "started") return WORK;
  if (state === "approved") return APPROVED;
  if (state === "denied") return DENIED;
  if (state === "attention") return [...ATTENTION, [timeline.repeat ? 4000 : 1600,REST] as Frame];
  return OTHER[state] ?? OTHER.idle;
}

const contourCache = new Map<string,{values:string;keyTimes:string;keySplines:string}>();

export function GeminiContourAnimation({ timeline }: { timeline: MascotTimeline | null | undefined }) {
  const ref = useRef<SVGAnimateElement>(null);
  const data = useMemo(() => {
    if (!timeline) return null;
    const cacheKey = `${timeline.state}:${timeline.repeat}`;
    const cached = contourCache.get(cacheKey);
    if (cached) return cached;
    const frames = gestureFrames(timeline);
    const end = frames[frames.length-1][0];
    const data = { values:frames.map(([,tips]) => geminiGesturePath(tips)).join(";"),
      keyTimes:frames.map(([time]) => time/end).join(";"),
      keySplines:frames.slice(1).map(() => ".42 0 .58 1").join(";") };
    contourCache.set(cacheKey,data);
    return data;
  }, [timeline?.state, timeline?.repeat]);
  useLayoutEffect(() => {
    if (!timeline || !ref.current) return;
    ref.current.beginElementAt?.(-timeline.phaseMs / 1000);
  }, [timeline?.key, timeline?.durationMs, timeline?.phaseMs]);
  if (!timeline || !data) return null;
  return <animate ref={ref} attributeName="d" begin="indefinite" dur={`${timeline.durationMs}ms`}
    values={data.values} keyTimes={data.keyTimes} keySplines={data.keySplines} calcMode="spline"
    repeatCount={timeline.repeat ? "indefinite" : 1} fill="remove" />;
}
