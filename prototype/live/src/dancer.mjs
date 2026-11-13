import { loadImage } from '../../src/canvas-engine.js';
import { danceAt } from './choreography.mjs';

// Landmarks in the original 1024 × 1536 portraits. X coordinates are the
// face, belt, and stance centers. The last two values are crown and sole Y.
const landmarks = {
  reach: [437, 490, 587, 10, 1527],
  rise: [449, 501, 373, 12, 1526],
  step: [396, 468, 637, 8, 1514],
  gather: [386, 441, 571, 7, 1511],
  open: [433, 515, 596, 9, 1516],
  sweep: [389, 453, 607, 12, 1511],
  wink: [393, 459, 627, 10, 1510],
  settle: [392, 453, 554, 10, 1518],
  front: [482, 490, 511, 10, 1523],
  'turn-left': [401, 505, 327, 11, 1529],
  'turn-right': [565, 520, 807, 12, 1518],
};

export async function loadDancer() {
  const poses = await Promise.all(Object.entries(landmarks).map(async ([name, [face, belt, feet, top, bottom]]) => {
    const image = await loadImage(`./assets/aoi-${name}.webp`);
    return [name, { image, face, belt, feet, top, bottom }];
  }));
  const layer = document.createElement('canvas');
  return { poses: Object.fromEntries(poses), layer, painter: layer.getContext('2d') };
}

const lerp = (a, b, t) => a + (b - a) * t;
const smooth = (t) => t * t * (3 - 2 * t);

export function drawDancer(ctx, dancer, layout, beat, ready, reducedMotion) {
  const { poses, layer, painter } = dancer;
  const { from, to, mix: progress, step } = ready ? { from: 'front', to: 'front', mix: 1, step: 0 } : danceAt(beat);
  const a = poses[from];
  const b = poses[to];
  const mix = reducedMotion ? Number(progress >= 0.5) : progress;
  const blend = smooth(Math.max(0, Math.min(1, (mix - 0.32) / 0.36)));
  const shift = reducedMotion ? 0 : step;
  const sway = reducedMotion ? 0 : Math.sin(beat * Math.PI);
  const bend = reducedMotion ? 0 : (1 - Math.cos(beat * Math.PI * 2)) * 0.005;
  const { body, center, floor } = layout;
  const x = center + shift * body * 0.16;
  const foot = x + (lerp(a.feet - a.belt, b.feet - b.belt, mix) / 1515) * body;

  ctx.save();
  ctx.translate(foot, floor);
  ctx.scale(body * 0.24, body * 0.033);
  const shadow = ctx.createRadialGradient(0, 0, 0, 0, 0, 1);
  shadow.addColorStop(0, '#08031aaa');
  shadow.addColorStop(1, '#08031a00');
  ctx.fillStyle = shadow;
  ctx.fillRect(-1, -1, 2, 2);
  ctx.restore();

  // Align faces and belts during a pose change. The lower edge stays at
  // the calibrated sole line, including during the beat-driven knee dip.
  const face = lerp(a.face - a.belt, b.face - b.belt, mix) + sway * 12;
  const referenceFeet = lerp(a.feet - a.belt, b.feet - b.belt, mix);
  const pixelRatio = ctx.getTransform().a;
  const size = Math.ceil(body * pixelRatio);
  if (layer.width !== size * 2 || layer.height !== size) {
    layer.width = size * 2;
    layer.height = size;
  }
  painter.clearRect(0, 0, layer.width, layer.height);
  painter.globalCompositeOperation = 'lighter';

  function paintPose(pose, opacity) {
    if (opacity <= 0) return;
    const sourceScale = pose.image.height / 1536;
    const unit = size / (pose.bottom - pose.top);
    const height = size * (1 - bend);
    painter.globalAlpha = opacity;
    const strips = 32;
    for (let strip = 0; strip < strips; strip++) {
      const start = Math.round(size - height + height * strip / strips);
      const end = Math.round(size - height + height * (strip + 1) / strips);
      const t = (strip + 0.5) / strips;
      const headWeight = 1 - smooth(Math.min(1, Math.max(0, (t - 0.16) / 0.25)));
      const footWeight = smooth(Math.min(1, Math.max(0, (t - 0.51) / 0.48)));
      const dx = (face - (pose.face - pose.belt)) * headWeight + (referenceFeet - (pose.feet - pose.belt)) * footWeight;
      const sourceY = (pose.top + (pose.bottom - pose.top) * strip / strips) * sourceScale;
      const sourceHeight = (pose.bottom - pose.top) * sourceScale / strips;
      painter.drawImage(pose.image, 0, sourceY, pose.image.width, sourceHeight,
        size + (dx - pose.belt) * unit, start, 1024 * unit, end - start);
    }
  }

  if (a === b) paintPose(a, 1);
  else {
    paintPose(a, 1 - blend);
    paintPose(b, blend);
  }
  painter.globalAlpha = 1;
  ctx.drawImage(layer, x - body, floor - body, body * 2, body);
}
