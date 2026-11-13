// Each cue is [beat, pose, horizontal step, transition length in beats].
const verse = [
  [0, 'front', 0, 0.6],
  [1, 'gather', -0.5, 0.72],
  [2, 'turn-left', -1, 0.76],
  [3.5, 'open', -0.45, 0.72],
  [4.5, 'front', 0, 0.66],
  [5.5, 'wink', 0.4, 0.56],
  [6.5, 'turn-right', 1, 0.8],
  [7.4, 'settle', 0.55, 0.54],
];
const chorus = [
  [0, 'front', 0, 0.55],
  [0.85, 'open', -0.25, 0.62],
  [1.7, 'sweep', -0.7, 0.62],
  [2.55, 'reach', -0.85, 0.65],
  [3.4, 'gather', -0.25, 0.7],
  [4.15, 'front', 0.15, 0.6],
  [4.9, 'wink', 0.45, 0.58],
  [5.65, 'step', 0.78, 0.64],
  [6.4, 'turn-right', 1, 0.64],
  [7.2, 'rise', 0.55, 0.71],
];
const cues = Array.from({ length: 16 }, (_, phrase) => {
  const at = phrase * 8;
  const steps = (at >= 40 && at < 72) || at >= 88 ? chorus : verse;
  return steps.map(([beat, pose, step, span]) => [at + beat, pose, step, span]);
}).flat();
cues.push([128, 'settle', 0.26, 0.95], [129.5, 'front', 0, 1.1]);

export function danceAt(beat) {
  beat = Math.max(0, beat);
  const next = cues.findIndex(([at]) => at > beat);
  const index = (next === -1 ? cues.length : next) - 1;
  const [at, to, end, span] = cues[index];
  const [, from, start] = cues[Math.max(0, index - 1)];
  const t = Math.min(1, Math.max(0, (beat - at) / span));
  const mix = t * t * (3 - 2 * t);
  return { from, to, mix, step: start + (end - start) * mix };
}

// The background is center-cropped with cover. Project the painted floor into the same viewport.
export function stageLayout(width, height, ready) {
  const scale = Math.max(width / 1600, height / 900);
  const floor = (height - 900 * scale) / 2 + 731 * scale;
  const mobile = width < 700;
  const body = Math.min(410 * scale, height * 0.52, width * (mobile ? 0.7 : 0.38));
  return {
    center: width * (ready ? (mobile ? 0.65 : 0.67) : 0.5),
    floor, body,
  };
}
