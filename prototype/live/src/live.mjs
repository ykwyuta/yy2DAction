import { startCanvasLoop } from '../../src/canvas-engine.js';
import { createRun } from './chart.mjs';
import { loadDancer } from './dancer.mjs';
import { BEAT, DURATION, VOCALS } from './song.mjs';
import { drawHighway, drawStage } from './stage.mjs';

const get = (id) => document.getElementById(id);
const game = get('game');
const stage = get('stage');
const chart = get('chart');
const stageContext = stage.getContext('2d');
const chartContext = chart.getContext('2d');
const start = get('start');
const status = get('load-status');
const sound = get('sound');
const pause = get('pause');
const overlay = get('overlay');
const overlayPrimary = get('overlay-primary');
const pads = Array.from(document.querySelectorAll('[data-lane]'));
const keys = ['KeyD', 'KeyF', 'KeyJ', 'KeyK'];
const flashes = [0, 0, 0, 0];
const motion = matchMedia('(prefers-reduced-motion: reduce)');
const labels = { perfect: 'PERFECT', great: 'GREAT', good: 'GOOD', miss: 'MISS' };
const scoreText = (score) => String(score).padStart(7, '0');
const track = new AudioContext();
const volume = track.createGain();
volume.gain.value = 0.8;
volume.connect(track.destination);

let dancer;
let song;
let source;
let clockOrigin = 0;
let time = 0;
let mode = 'ready';
let run;
let easy;
let judgmentUntil = 0;
let busy = false;
let muted = false;

function changeMode(next) {
  mode = next;
  game.dataset.mode = next;
  pause.disabled = next !== 'playing';
}

function stopAudio() {
  if (!source) return;
  source.stop();
  source.disconnect();
  source = undefined;
}

async function play(restart) {
  if (busy || !song) return;
  busy = true;
  try {
    await track.resume();
    if (document.hidden) return;
    if (restart) {
      easy = document.querySelector('input[name="difficulty"]:checked').value === 'easy';
      run = createRun(easy);
      time = 0;
      flashes.fill(0);
      judgmentUntil = 0;
    }
    stopAudio();
    source = track.createBufferSource();
    source.buffer = song;
    source.connect(volume);
    const beginAt = track.currentTime + 0.035;
    source.start(beginAt, time);
    clockOrigin = beginAt - time;
    if (overlay.open) overlay.close();
    changeMode('playing');
    pause.focus({ preventScroll: true });
  } catch (error) {
    console.error(error);
    if (mode === 'ready') status.textContent = '音声を再生できませんでした。もう一度お試しください。';
    else get('overlay-title').textContent = '音声を再生できませんでした';
  } finally {
    busy = false;
  }
}

function clock() {
  return Math.max(0, track.currentTime - clockOrigin);
}

function showOverlay(label, title, action, showResults) {
  get('overlay-label').textContent = label;
  get('overlay-title').textContent = title;
  overlayPrimary.textContent = action;
  get('results').hidden = !showResults;
  overlay.showModal();
  overlayPrimary.focus();
}

function pauseGame() {
  if (mode !== 'playing') return;
  time = Math.min(DURATION, clock());
  if (time >= DURATION) {
    finish();
    return;
  }
  stopAudio();
  changeMode('paused');
  showOverlay('PAUSE', 'ライブを一時停止', '再開する', false);
}

function finish() {
  run.expire(Infinity);
  stopAudio();
  changeMode('result');
  const accuracy = run.accuracy;
  get('rank').textContent = accuracy >= .95 ? 'S' : accuracy >= .85 ? 'A' : accuracy >= .7 ? 'B' : accuracy >= .5 ? 'C' : 'D';
  get('final-score').textContent = scoreText(run.score);
  const entries = Object.entries(run.counts).map(([key, value]) => [labels[key], value]);
  entries.push(['MAX COMBO', run.best]);
  get('breakdown').replaceChildren(...entries.map(([label, count]) => {
    const item = document.createElement('div');
    const term = document.createElement('dt');
    const value = document.createElement('dd');
    term.textContent = label;
    value.textContent = count;
    item.append(term, value);
    return item;
  }));
  showOverlay('LIVE RESULT', run.best === run.notes.length ? 'フルコンボ！' : 'ライブ終了', 'もう一度プレイ', true);
}

function back() {
  if (busy) return;
  overlay.close();
  run = undefined;
  time = 0;
  changeMode('ready');
  start.focus();
}

function showJudgment(result, now) {
  get('judgment').textContent = labels[result];
  judgmentUntil = now + 490;
}

function tap(lane) {
  if (mode !== 'playing') return;
  const now = performance.now();
  flashes[lane] = now + 170;
  const previousMisses = run.counts.miss;
  const result = run.hit(lane, clock());
  if (result || run.counts.miss > previousMisses) showJudgment(result || 'miss', now);
}

function frame(now) {
  if (mode === 'playing') {
    time = Math.min(DURATION, clock());
    const previousMisses = run.counts.miss;
    run.expire(time);
    if (run.counts.miss > previousMisses) showJudgment('miss', now);
    if (time >= DURATION) finish();
  }

  const ready = mode === 'ready';
  const beat = time / BEAT;
  drawStage(stage, stageContext, dancer, ready ? now / 1300 : beat, ready, motion.matches);
  if (!ready) drawHighway(chart, chartContext, run.notes, time, flashes, now);
  pads.forEach((pad, lane) => pad.toggleAttribute('data-hit', flashes[lane] > now && mode === 'playing'));
  get('judgment').hidden = judgmentUntil <= now;
  get('score').textContent = scoreText(run?.score || 0);
  get('combo').textContent = run?.combo || 0;
  const phrase = !ready && VOCALS.find(([startBeat, , , , lengths]) => {
    return beat >= startBeat && beat < startBeat + lengths.reduce((sum, length) => sum + length, 0);
  });
  const lyric = phrase ? phrase[1] : '';
  if (get('lyric').textContent !== lyric) get('lyric').textContent = lyric;
  get('progress').value = time;
  const elapsed = mode === 'result' ? Math.ceil(DURATION) : Math.floor(time);
  get('elapsed').textContent = `${Math.floor(elapsed / 60)}:${String(elapsed % 60).padStart(2, '0')}`;
}

start.addEventListener('click', () => play(true));
pause.addEventListener('click', pauseGame);
overlayPrimary.addEventListener('click', () => play(mode === 'result'));
get('back').addEventListener('click', back);
overlay.addEventListener('cancel', (event) => {
  event.preventDefault();
  if (mode === 'paused') play(false);
});
sound.addEventListener('click', () => {
  muted = !muted;
  volume.gain.setTargetAtTime(muted ? 0 : 0.8, track.currentTime, 0.02);
  sound.setAttribute('aria-pressed', String(muted));
  sound.setAttribute('aria-label', muted ? 'ミュートを解除' : '音声をミュート');
  sound.querySelector('span').textContent = muted ? 'ミュート中' : 'サウンド';
});
pads.forEach((pad, lane) => {
  pad.addEventListener('pointerdown', (event) => {
    if (event.pointerType === 'mouse' && event.button !== 0) return;
    event.preventDefault();
    tap(lane);
  });
  pad.addEventListener('click', (event) => {
    if (event.detail === 0) tap(lane);
  });
});
document.addEventListener('keydown', (event) => {
  if (event.repeat || mode !== 'playing') return;
  const lane = keys.indexOf(event.code);
  if (lane !== -1) {
    event.preventDefault();
    tap(lane);
  } else if (event.code === 'Escape') {
    event.preventDefault();
    pauseGame();
  }
});
document.addEventListener('visibilitychange', () => {
  if (document.hidden) pauseGame();
});

startCanvasLoop(frame);

async function loadSong() {
  const response = await fetch('./assets/kirari-hikaru.mp3');
  if (!response.ok) throw new Error(`Song load failed (${response.status})`);
  return track.decodeAudioData(await response.arrayBuffer());
}

async function prepare() {
  [dancer, song] = await Promise.all([loadDancer(), loadSong()]);
  status.textContent = '';
  start.disabled = false;
}

prepare().catch((error) => {
  console.error(error);
  status.textContent = '素材を読み込めませんでした。ページを再読み込みしてください。';
});
