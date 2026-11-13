export function startCanvasLoop(draw) {
  let lastTime;
  function frame(now) {
    const delta = lastTime === undefined ? 0 : Math.min(0.05, (now - lastTime) / 1000);
    lastTime = now;
    draw(now, delta);
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
}

export async function loadImage(url) {
  const image = new Image();
  image.src = url;
  await image.decode();
  return image;
}

export function fitCanvas(canvas, context) {
  const bounds = canvas.getBoundingClientRect();
  const scale = Math.min(devicePixelRatio || 1, 2);
  const width = Math.round(bounds.width * scale);
  const height = Math.round(bounds.height * scale);
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  context.setTransform(scale, 0, 0, scale, 0, 0);
  return bounds;
}
