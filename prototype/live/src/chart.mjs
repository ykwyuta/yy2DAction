import { BEAT } from './song.mjs';

const patterns = [
  [[0, 0], [1, 1], [2, 2], [3, 3]],
  [[0, 3], [1, 2], [2, 1], [3, 0], [3.5, 1]],
  [[0, 0], [0.5, 1], [1.5, 2], [2, 3], [3, 1]],
  [[0, 2], [1, 1], [1.5, 2], [2, 0], [3, 3]],
  [[0, 0], [0, 3], [1, 1], [1.5, 2], [2.5, 3], [3, 1], [3.5, 2]],
  [[0, 3], [0.5, 2], [1, 1], [2, 0], [2, 3], [3, 2]],
  [[0, 0], [0.5, 1], [1, 2], [1.5, 3], [2, 2], [2.5, 1], [3, 0]],
  [[0, 1], [0, 2], [1, 0], [1.5, 2], [2, 1], [3, 0], [3, 3]],
];

export const WINDOWS = { perfect: 0.065, great: 0.13, good: 0.19 };
const points = { perfect: 1, great: 0.8, good: 0.5, miss: 0 };

export function makeChart(easy) {
  const notes = [];
  for (let bar = 1; bar < 32; bar++) {
    const chorus = (bar >= 10 && bar < 18) || bar >= 22;
    const pattern = patterns[bar % 4 + (chorus ? 4 : 0)];
    for (const [beat, lane] of pattern) {
      if (easy && (!Number.isInteger(beat) || notes.at(-1)?.time === (bar * 4 + beat) * BEAT)) continue;
      notes.push({ time: (bar * 4 + beat) * BEAT, lane, judged: false });
    }
  }
  return notes;
}

export function createRun(easy) {
  const notes = makeChart(easy);
  let first = 0;
  const counts = { perfect: 0, great: 0, good: 0, miss: 0 };
  const run = {
    notes, counts, combo: 0, best: 0, score: 0,
    get accuracy() {
      return Object.entries(counts).reduce((sum, [kind, count]) => sum + count * points[kind], 0) / notes.length;
    },
    expire(time) {
      while (notes[first] && notes[first].time < time - WINDOWS.good) {
        const note = notes[first++];
        if (!note.judged) register(note, 'miss');
      }
    },
    hit(lane, time) {
      run.expire(time);
      let target;
      let error = Infinity;
      for (let i = first; i < notes.length && notes[i].time <= time + WINDOWS.good; i++) {
        const note = notes[i];
        const distance = Math.abs(note.time - time);
        if (note.lane === lane && !note.judged && distance < error) {
          target = note;
          error = distance;
        }
      }
      if (!target) return null;
      const result = error <= WINDOWS.perfect ? 'perfect' : error <= WINDOWS.great ? 'great' : 'good';
      register(target, result);
      return result;
    },
  };

  function register(note, result) {
    note.judged = true;
    counts[result]++;
    run.combo = result === 'miss' ? 0 : run.combo + 1;
    run.best = Math.max(run.best, run.combo);
    run.score = Math.round(run.accuracy * 1_000_000);
  }

  return run;
}
