import { writeFileSync } from 'node:fs';
import { BEAT, SONG_BEATS, VOCALS } from './src/song.mjs';

const output = process.argv[2];
if (!output) throw new Error('Usage: node render-song.mjs <output.wav>');

const rate = 32_000;
const mix = new Float32Array(Math.round(rate * BEAT * SONG_BEATS));
const tau = 2 * Math.PI;
const hz = (midi) => 440 * 2 ** ((midi - 69) / 12);
let seed = 26;
function noise() {
  seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
  return seed / 2147483648 - 1;
}
function add(beat, seconds, voice, gain, echo = 0) {
  const start = Math.round(beat * BEAT * rate);
  const delay = Math.round(rate * 0.18);
  for (let i = 0; i < seconds * rate && start + i < mix.length; i++) {
    const value = voice(i / rate) * gain;
    mix[start + i] += value;
    if (echo && start + i + delay < mix.length) mix[start + i + delay] += value * echo;
  }
}

function synth(beat, pitch, length, gain, kind) {
  const frequency = hz(pitch);
  add(beat, length, (t) => {
    const phase = tau * t * frequency;
    const envelope = Math.min(1, t / 0.017, (length - t) / 0.085);
    if (kind === 'pad') return envelope * (Math.sin(phase) + 0.4 * Math.sin(phase * 1.004) + 0.2 * Math.sin(phase * 2));
    if (kind === 'bass') return envelope * (Math.sin(phase) + 0.27 * Math.sin(phase * 2)) * Math.exp(-2 * t);
    return envelope * (Math.sin(phase) + 0.45 * Math.sin(phase * 2.01) + 0.16 * Math.sin(phase * 3)) * Math.exp(-8 * t);
  }, gain, kind === 'bell' ? 0.32 : 0);
}

const chords = [[52, 56, 59, 63], [47, 51, 54, 57], [49, 52, 56, 59], [45, 49, 52, 56]];
for (let bar = 0; bar < SONG_BEATS / 4; bar++) {
  const chord = chords[bar % chords.length];
  const start = bar * 4;
  const ending = bar === 32;
  const breakBar = bar === 18 || bar === 19;
  for (const pitch of chord) synth(start, pitch + 12, BEAT * 4, 0.025, 'pad');
  if (ending) continue;
  for (let beat = 0; beat < 4; beat++) {
    const time = start + beat;
    synth(time, chord[0] - 12 + (beat === 3 ? 12 : 0), BEAT * 0.91, 0.19, 'bass');
    synth(time + 0.5, chord[(beat + bar) % 4] + 24, BEAT * 0.9, 0.06, 'bell');
    if (breakBar) continue;
    add(time, 0.23, (t) => Math.sin(tau * (52 * t + 3 * (1 - Math.exp(-45 * t)))) * Math.exp(-17 * t), 0.32);
    if (beat % 2) add(time, 0.15, (t) => (noise() * 0.66 + Math.sin(tau * t * 172) * 0.33) * Math.exp(-27 * t), 0.18, 0.1);
    add(time + 0.5, 0.11, (t) => noise() * Math.exp(-37 * t), 0.115);
  }
}

// Vowels use a harmonic glottal source filtered by the first three formants.
const formants = {
  a: [820, 1430, 2800], i: [320, 2350, 3250], u: [340, 1070, 2440],
  e: [490, 2050, 2900], o: [490, 950, 2660],
};
const widths = [125, 180, 270];
function sing(beat, mora, pitch, length) {
  const frequency = hz(pitch);
  const vowel = mora.at(-1);
  const onset = mora.length === 1 ? 0 : mora.startsWith('tt') ? 0.082 : 0.048;
  const consonant = mora.slice(0, -1);
  const harmonics = Array.from({ length: 16 }, (_, i) => {
    const partial = (i + 1) * frequency;
    const weight = formants[vowel].reduce((total, center, formant) => {
      return total + [1, 0.52, 0.23][formant] * Math.exp(-0.5 * ((partial - center) / widths[formant]) ** 2);
    }, 0.27);
    return weight / (i + 1) ** 0.72;
  });
  const normalization = harmonics.reduce((a, b) => a + b, 0);
  let phase = 0;
  add(beat, length, (t) => {
    if (t < onset) {
      const envelope = Math.sin(Math.PI * t / onset);
      if (/^[mnrwy]/.test(consonant)) return envelope * Math.sin(tau * frequency * t) * 0.26;
      return envelope * noise() * (/^(s|sh|h|ts|j)/.test(consonant) ? 0.55 : 0.3);
    }
    const age = t - onset;
    const vibrato = Math.min(1, age * 3) * Math.sin(tau * 5.3 * age);
    phase += tau * frequency * (1 - 0.045 * Math.exp(-age * 46) + 0.012 * vibrato) / rate;
    const tone = harmonics.reduce((sum, weight, i) => sum + weight * Math.sin((i + 1) * phase), 0);
    const envelope = Math.max(0, Math.min(1, age / 0.024, (length - t) / 0.065));
    return envelope * (tone / normalization + 0.025 * noise());
  }, 0.53, 0.22);
}

for (const [start, , lyrics, pitches, lengths] of VOCALS) {
  let beat = start;
  lyrics.split(' ').forEach((mora, i) => {
    sing(beat, mora, pitches[i], lengths[i] * BEAT - 0.012);
    beat += lengths[i];
  });
}

const wave = Buffer.alloc(44 + mix.length * 2);
wave.write('RIFF', 0);
wave.writeUInt32LE(wave.length - 8, 4);
wave.write('WAVEfmt ', 8);
wave.writeUInt32LE(16, 16);
wave.writeUInt16LE(1, 20);
wave.writeUInt16LE(1, 22);
wave.writeUInt32LE(rate, 24);
wave.writeUInt32LE(rate * 2, 28);
wave.writeUInt16LE(2, 32);
wave.writeUInt16LE(16, 34);
wave.write('data', 36);
wave.writeUInt32LE(wave.length - 44, 40);
for (let i = 0; i < mix.length; i++) {
  const fade = Math.min(1, i / (rate * 0.03), (mix.length - i) / (rate * 0.6));
  wave.writeInt16LE(Math.round(Math.tanh(mix[i] * 1.65) * fade * 32766), 44 + i * 2);
}
writeFileSync(output, wave);
