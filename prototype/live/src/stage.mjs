import { fitCanvas } from '../../src/canvas-engine.js';

const choreography = [2, 0, 2, 1, 2, 1, 0, 1];
const colors = ['#fc8fd6', '#c1a6ff', '#8ce9ff', '#f8bbff'];
const approach = 1.7;

export function drawStage(canvas, ctx, poses, beat, ready, reducedMotion) {
  const { width, height } = fitCanvas(canvas, ctx);
  ctx.clearRect(0, 0, width, height);
  const mobile = width < 700;
  const pulse = reducedMotion ? 0 : Math.exp(-5 * (beat % 1));
  const swing = reducedMotion ? 0 : Math.sin(beat * Math.PI);
  const particleBeat = reducedMotion ? 0 : beat;
  const center = width * (ready ? (mobile ? 0.62 : 0.67) : 0.5);
  const foot = height * (ready ? (mobile ? 0.89 : 0.99) : (mobile ? 0.73 : 0.78));

  ctx.save();
  ctx.globalCompositeOperation = 'screen';
  const bloom = ctx.createRadialGradient(center, height * 0.45, 10, center, height * 0.45, height * 0.64);
  bloom.addColorStop(0, `rgba(224,103,247,${0.19 + pulse * 0.15})`);
  bloom.addColorStop(1, 'rgba(113,40,215,0)');
  ctx.fillStyle = bloom;
  ctx.fillRect(0, 0, width, height);
  for (let i = 0; i < 26; i++) {
    const y = ((i * 0.263 + particleBeat * 0.022) % 1) * height;
    const x = ((i * 0.618 + Math.sin(i + particleBeat * 0.18) * 0.025) % 1) * width;
    ctx.globalAlpha = 0.2 + pulse * 0.25;
    ctx.fillStyle = colors[i % colors.length];
    ctx.fillRect(x, y, 2 + i % 3, 2 + i % 3);
  }
  ctx.restore();
  if (!poses.length) return;

  ctx.save();
  ctx.translate(center, foot);
  ctx.fillStyle = '#08051f80';
  ctx.beginPath();
  ctx.ellipse(0, 1, height * 0.17, height * 0.022, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.translate(ready ? 0 : swing * 10, -pulse * (ready ? 1 : 9));
  ctx.rotate(swing * (ready ? 0.004 : 0.034));
  ctx.shadowColor = '#2d104d';
  ctx.shadowBlur = 18;
  const pose = poses[ready ? 0 : choreography[Math.floor(beat / 2) % choreography.length]];
  const spriteHeight = height * (ready ? (mobile ? 0.82 : 0.95) : 0.7);
  const spriteWidth = spriteHeight * pose.width / pose.height;
  ctx.drawImage(pose, -spriteWidth * 0.47, -spriteHeight, spriteWidth, spriteHeight);
  ctx.restore();
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
  floor.addColorStop(0.5, '#160d29a8');
  floor.addColorStop(1, '#0e091eef');
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
