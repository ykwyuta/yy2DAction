import { fitCanvas } from '../../src/canvas-engine.js';
import { stageLayout } from './choreography.mjs';
import { drawDancer } from './dancer.mjs';

const colors = ['#fc8fd6', '#c1a6ff', '#8ce9ff', '#f8bbff'];
const approach = 1.7;

export function drawStage(canvas, ctx, dancer, beat, ready, reducedMotion) {
  const { width, height } = fitCanvas(canvas, ctx);
  if (!width || !height) return;
  ctx.clearRect(0, 0, width, height);
  const layout = stageLayout(width, height, ready);
  const pulse = reducedMotion ? 0 : Math.exp(-5 * (beat % 1));
  const particleBeat = reducedMotion ? 0 : beat;

  ctx.save();
  ctx.globalCompositeOperation = 'screen';
  const lightY = layout.floor - layout.body * 0.63;
  const bloom = ctx.createRadialGradient(layout.center, lightY, 0, layout.center, lightY, height * 0.6);
  bloom.addColorStop(0, `rgba(224,103,247,${0.19 + pulse * 0.15})`);
  bloom.addColorStop(1, 'rgba(113,40,215,0)');
  ctx.fillStyle = bloom;
  ctx.fillRect(0, 0, width, height);
  for (let i = 0; i < 26; i++) {
    const y = ((i * 0.263 + particleBeat * 0.022) % 1) * height;
    const x = ((1 + i * 0.618 + Math.sin(i + particleBeat * 0.18) * 0.025) % 1) * width;
    ctx.globalAlpha = 0.2 + pulse * 0.25;
    ctx.fillStyle = colors[i % colors.length];
    ctx.fillRect(x, y, 2 + i % 3, 2 + i % 3);
  }
  ctx.restore();
  if (dancer) drawDancer(ctx, dancer, layout, beat, ready, reducedMotion);
}

export function drawHighway(canvas, ctx, notes, time, flashes, now) {
  const { width, height } = fitCanvas(canvas, ctx);
  ctx.clearRect(0, 0, width, height);
  if (!width || !height) return;
  const hitY = height - 15;
  const xAt = (column, depth) => width / 2 + (column - 2) * width / 4 * (0.34 + 0.66 * depth);
  const yAt = (depth) => hitY * depth;
  const floor = ctx.createLinearGradient(0, 0, 0, hitY);
  floor.addColorStop(0, '#160d2908');
  floor.addColorStop(0.58, '#160d295c');
  floor.addColorStop(1, '#0e091edc');
  ctx.fillStyle = floor;
  ctx.beginPath();
  ctx.moveTo(xAt(0, 0), 0);
  ctx.lineTo(xAt(4, 0), 0);
  ctx.lineTo(width, hitY);
  ctx.lineTo(0, hitY);
  ctx.closePath();
  ctx.fill();
  for (let column = 0; column <= 4; column++) {
    ctx.strokeStyle = column === 0 || column === 4 ? '#dcb3fbba' : '#c3a0ec69';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(xAt(column, 0), 0);
    ctx.lineTo(xAt(column, 1), hitY);
    ctx.stroke();
  }

  for (const note of notes) {
    if (note.time > time + approach) break;
    if (note.judged) continue;
    const depth = 1 - (note.time - time) / approach;
    const projected = depth ** 1.55;
    const x = xAt(note.lane + 0.5, Math.min(1, projected));
    const w = width / 4 * (0.27 + 0.53 * Math.min(1, projected));
    const y = yAt(projected);
    const h = 4 + 11 * Math.min(1, projected);
    ctx.shadowColor = colors[note.lane];
    ctx.shadowBlur = 13;
    ctx.fillStyle = colors[note.lane];
    ctx.beginPath();
    ctx.roundRect(x - w / 2, y - h / 2, w, h, h / 2);
    ctx.fill();
    ctx.shadowBlur = 0;
    ctx.fillStyle = '#fff9';
    ctx.fillRect(x - w * 0.31, y - h * 0.24, w * 0.62, 1.5);
  }

  for (let lane = 0; lane < 4; lane++) {
    const x = xAt(lane, 1) + 4;
    const w = width / 4 - 8;
    const intensity = Math.max(0, (flashes[lane] - now) / 170);
    ctx.strokeStyle = colors[lane];
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.roundRect(x, hitY - 8, w, 16, 5);
    ctx.stroke();
    ctx.globalAlpha = 0.14 + intensity * 0.75;
    ctx.fillStyle = colors[lane];
    ctx.fill();
    ctx.globalAlpha = 1;
  }
}
