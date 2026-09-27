// Background music through Web Audio rather than an audio element: it loops
// sample-accurately, fades, and its volume works on iPhone, where a media
// element's volume cannot be set from script.

/** Slider position to gain: loudness is heard roughly as the square. */
const gainOf = (volume: number) => volume * volume;
/** Fades in and out take about this long, in seconds. */
const FADE = 0.9;

type AudioSessionNavigator = Navigator & { audioSession?: { type: string } };

export class Music {
	private ctx?: AudioContext;
	private gain?: GainNode;
	private loading?: Promise<void>;
	/** Bumped by every play or stop, so a stale stop cannot suspend a replay. */
	private generation = 0;

	constructor(private readonly url: string) {}

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

	private async load(ctx: AudioContext, gain: GainNode): Promise<void> {
		const response = await fetch(this.url);
		if (!response.ok) throw new Error(`music: ${response.status} for ${this.url}`);
		const buffer = await ctx.decodeAudioData(await response.arrayBuffer());
		// Some decoders keep encoder padding as exact silence at either end. The
		// track is never silent there (it loops through a crossfade), so any
		// run of true zeros at the edges is padding, and the loop skips it.
		const samples = buffer.getChannelData(0);
		let head = 0;
		while (head < 8192 && samples[head] === 0) head++;
		let tail = samples.length;
		while (tail > samples.length - 8192 && samples[tail - 1] === 0) tail--;
		const source = ctx.createBufferSource();
		source.buffer = buffer;
		source.loop = true;
		source.loopStart = head / buffer.sampleRate;
		source.loopEnd = tail / buffer.sampleRate;
		source.connect(gain);
		source.start(0, source.loopStart);
	}
}
