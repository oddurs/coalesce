// Background music through Web Audio rather than an audio element: it loops
// sample-accurately, fades, and its volume works on iPhone, where a media
// element's volume cannot be set from script.

/** Slider position to gain: loudness is heard roughly as the square. */
const gainOf = (volume: number) => volume * volume;
/** Fades in and out take about this long, in seconds. */
const FADE = 0.9;
/** The opening crossfades into the whole track over this long, in seconds. */
const HANDOVER = 0.12;

/** A buffer playing on a loop through its own gain, from `startedAt` on the context clock. */
type Loop = { source: AudioBufferSourceNode; level: GainNode; startedAt: number; length: number };

type AudioSessionNavigator = Navigator & { audioSession?: { type: string } };

export class Music {
	private ctx?: AudioContext;
	private gain?: GainNode;
	private loading?: Promise<void>;
	private opening?: Promise<ArrayBuffer>;
	/** Bumped by every play or stop, so a stale stop cannot suspend a replay. */
	private generation = 0;

	/**
	 * `headUrl` holds the track's opening seconds on their own. Decoding the
	 * whole track takes over a second and it can queue behind the film's
	 * download, so sound starts on the opening and hands over when the whole
	 * track is ready.
	 */
	constructor(
		private readonly url: string,
		private readonly headUrl: string
	) {}

	/**
	 * Take the opening ahead of any click. The page preloads it from the HTML,
	 * so this picks up that response. Safe to call again.
	 */
	prefetch(): void {
		this.opening ??= bytes(this.headUrl);
		// A failed prefetch surfaces in load(), which falls back to the whole track.
		this.opening.catch(() => {});
	}

	/**
	 * Start, loading the track on first use, and fade to `volume`. Call it from
	 * a user gesture: browsers only let audio start from one.
	 */
	async play(volume: number): Promise<void> {
		this.generation++;
		if (!this.ctx) {
			// iOS files Web Audio under ambient sound, which the ring switch
			// silences; this is music someone asked for, so ask for playback.
			const session = (navigator as AudioSessionNavigator).audioSession;
			if (session) session.type = 'playback';
			this.ctx = new AudioContext();
			this.gain = this.ctx.createGain();
			this.gain.gain.value = 0;
			this.gain.connect(this.ctx.destination);
			this.loading = this.load(this.ctx, this.gain);
		}
		// Resume before any await, while still inside the gesture.
		const resumed = this.ctx.resume();
		try {
			await this.loading;
		} catch (error) {
			// Forget the failed context so the next attempt starts clean.
			this.ctx.close();
			this.ctx = undefined;
			throw error;
		}
		await resumed;
		this.fade(gainOf(volume), FADE);
	}

	/** Follow the slider while it moves. */
	set(volume: number): void {
		this.fade(gainOf(volume), 0.08);
	}

	/** Fade out, then suspend so an idle context costs nothing. */
	async stop(): Promise<void> {
		if (!this.ctx) return;
		const generation = ++this.generation;
		this.fade(0, FADE);
		await new Promise((done) => setTimeout(done, FADE * 1500));
		if (generation === this.generation) await this.ctx.suspend();
	}

	private fade(target: number, seconds: number): void {
		if (!this.ctx || !this.gain) return;
		const now = this.ctx.currentTime;
		const level = this.gain.gain;
		level.cancelScheduledValues(now);
		level.setValueAtTime(level.value, now);
		level.setTargetAtTime(target, now, seconds / 3);
	}

	/** Resolves once sound is playing: the opening, or the whole track without one. */
	private async load(ctx: AudioContext, gain: GainNode): Promise<void> {
		this.prefetch();
		const whole = bytes(this.url).then((data) => ctx.decodeAudioData(data));
		let opening: Loop;
		try {
			opening = loop(ctx, gain, await ctx.decodeAudioData(await this.opening!), 0);
		} catch {
			// No opening (not deployed, or it failed): wait for the whole track.
			loop(ctx, gain, await whole, 0);
			return;
		}
		whole.then(
			(buffer) => handOver(ctx, gain, opening, buffer),
			// The whole track failed: the opening keeps looping on its own.
			() => {}
		);
	}
}

async function bytes(url: string): Promise<ArrayBuffer> {
	const response = await fetch(url);
	if (!response.ok) throw new Error(`music: ${response.status} for ${url}`);
	return response.arrayBuffer();
}

/** Play `buffer` on a loop from `offset` seconds into the track, starting now. */
function loop(ctx: AudioContext, out: GainNode, buffer: AudioBuffer, offset: number, at = 0): Loop {
	// Some decoders keep encoder padding as exact silence at either end. The
	// track is never silent there (it loops through a crossfade), so any run
	// of true zeros at the edges is padding, and the loop skips it. Track
	// time starts at the first real sample, which lines the two files up.
	const samples = buffer.getChannelData(0);
	let head = 0;
	while (head < 8192 && samples[head] === 0) head++;
	let tail = samples.length;
	while (tail > samples.length - 8192 && samples[tail - 1] === 0) tail--;
	const level = ctx.createGain();
	level.connect(out);
	const source = ctx.createBufferSource();
	source.buffer = buffer;
	source.loop = true;
	source.loopStart = head / buffer.sampleRate;
	source.loopEnd = tail / buffer.sampleRate;
	source.connect(level);
	const startedAt = Math.max(at, ctx.currentTime);
	source.start(startedAt, source.loopStart + offset);
	return {
		source,
		level,
		startedAt: startedAt - offset,
		length: source.loopEnd - source.loopStart
	};
}

/** Crossfade from the opening into the whole track at the same point in the music. */
function handOver(ctx: AudioContext, out: GainNode, opening: Loop, buffer: AudioBuffer): void {
	let at = ctx.currentTime + 0.05;
	let offset = (at - opening.startedAt) % opening.length;
	// Never straddle the opening's own wrap: hand over just after it instead.
	if (offset > opening.length - HANDOVER - 0.02) {
		at += opening.length - offset;
		offset = 0;
	}
	const next = loop(ctx, out, buffer, offset, at);
	next.level.gain.setValueAtTime(0, at);
	next.level.gain.linearRampToValueAtTime(1, at + HANDOVER);
	opening.level.gain.setValueAtTime(1, at);
	opening.level.gain.linearRampToValueAtTime(0, at + HANDOVER);
	opening.source.stop(at + HANDOVER);
}
