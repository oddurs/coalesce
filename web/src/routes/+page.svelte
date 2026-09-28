<script lang="ts">
	import { onMount } from 'svelte';
	import manifest from '$lib/media.json';

	// The web set lives on R2 under names that carry a hash of the content;
	// scripts/publish uploads it and writes the manifest this reads.
	const files: Record<string, string> = manifest.files;
	const media = (name: string) => `${manifest.origin}/${files[name]}`;

	// The browser picks the first source it can decode whose media query
	// matches, straight from the HTML, so the film starts loading before any
	// script has run. Phones get the small encode: full 2K is wasted on a small
	// screen and costs the most bandwidth. Judged by the screen's short side,
	// since a large phone held sideways is wider than many laptops. With
	// reduced motion nothing matches and nothing downloads; the page is a still.
	const motion = '(prefers-reduced-motion: no-preference)';
	const phone = `${motion} and (max-device-width: 600px), ${motion} and (max-device-height: 600px)`;
	const small = media('loop-720.mp4');
	const h264 = 'video/mp4; codecs="avc1.64001f"';
	const sources = [
		[small, h264, phone],
		[media('loop-av1.mp4'), 'video/mp4; codecs="av01.0.08M.10"', motion],
		[media('loop-hevc.mp4'), 'video/mp4; codecs="hvc1.2.4.L120.B0"', motion],
		[small, h264, motion]
	];
	const IDLE_MS = 2500;
	// The title and controls hold a little longer the first time, as the film
	// fades up, like an opening card.
	const INTRO_MS = 4000;

	type IosVideo = HTMLVideoElement & { webkitEnterFullscreen?: () => void };

	let main: HTMLElement;
	let video: IosVideo;
	let paused = $state(true);
	// Nothing shows until the film can play; then it fades up from black.
	let ready = $state(false);
	// Waiting on the network mid-film. The loader comes back only if the wait
	// outlasts a moment, so a seek does not flash it.
	let stalled = $state(false);
	let stall: ReturnType<typeof setTimeout> | undefined;
	let idle = $state(false);
	let fullscreen = $state(false);
	let progress = $state(0);
	let duration = $state(0);
	// How far the browser has the film downloaded ahead of the playhead.
	let buffered = $state(0);
	let timer: ReturnType<typeof setTimeout> | undefined;

	// Scrubbing: a press on the track seeks there at once, a drag follows the
	// pointer with playback held, and the release lands exactly and resumes.
	let track: HTMLElement;
	let scrubbing = $state(false);
	let resumeAfterScrub = false;
	// Where the mouse hovers over the track, for the time it would jump to.
	let hoverAt = $state<number | null>(null);

	// The film carries its score. It plays muted until asked: browsers only
	// start sound from a gesture, and nobody wants a page that shouts.
	const VOLUME_KEY = 'coalesce:volume';
	const SOUND_KEY = 'coalesce:sound';
	let sound = $state(false);
	let volume = $state(0.5);
	// iPhone gives script no say over a video's volume; its buttons own it.
	// There the slider is hidden and the speaker only mutes.
	let fixedVolume = $state(false);
	let ramp = 0;

	onMount(() => {
		// The film began loading from the HTML before this ran: catch up on
		// what it did meanwhile. With reduced motion there is nothing to wait
		// for; with every source already failed, the poster is all there is.
		paused = video.paused;
		const failed =
			video.networkState === video.NETWORK_NO_SOURCE &&
			video.readyState === video.HAVE_NOTHING;
		if (!matchMedia(motion).matches || failed) ready = true;
		else if (!video.paused && video.readyState >= video.HAVE_FUTURE_DATA) ready = true;
		else if (video.readyState >= video.HAVE_CURRENT_DATA) play();

		// Data Saver asks for the small encode on any screen.
		const saveData = (navigator as { connection?: { saveData?: boolean } }).connection
			?.saveData;
		if (saveData && matchMedia(motion).matches && !video.currentSrc.endsWith(small)) {
			video.src = small;
		}

		// A returning listener's volume, and whether they had sound on. Storage
		// can be unavailable (private windows, blocked cookies); then defaults.
		let wanted = false;
		try {
			const saved = Number(localStorage.getItem(VOLUME_KEY));
			if (saved > 0 && saved <= 1) volume = saved;
			wanted = localStorage.getItem(SOUND_KEY) === '1';
		} catch {
			wanted = false;
		}
		// Sound needs a gesture, so it comes back on the first tap or key.
		const resume = () => {
			if (wanted && !sound) startSound();
		};
		if (wanted) {
			window.addEventListener('pointerdown', resume, { once: true });
			window.addEventListener('keydown', resume, { once: true });
		}

		video.volume = 0.5;
		fixedVolume = video.volume !== 0.5;

		const onFullscreen = () => (fullscreen = document.fullscreenElement !== null);
		document.addEventListener('fullscreenchange', onFullscreen);
		let frame = requestAnimationFrame(function tick() {
			if (video.duration) {
				duration = video.duration;
				if (!scrubbing) progress = video.currentTime / video.duration;
				const now = video.currentTime;
				for (let i = 0; i < video.buffered.length; i++) {
					if (video.buffered.start(i) <= now + 0.5 && now <= video.buffered.end(i)) {
						buffered = video.buffered.end(i) / video.duration;
					}
				}
			}
			frame = requestAnimationFrame(tick);
		});
		return () => {
			window.removeEventListener('pointerdown', resume);
			window.removeEventListener('keydown', resume);
			document.removeEventListener('fullscreenchange', onFullscreen);
			cancelAnimationFrame(frame);
			cancelAnimationFrame(ramp);
			clearTimeout(timer);
			clearTimeout(stall);
		};
	});

	function waiting() {
		clearTimeout(stall);
		stall = setTimeout(() => (stalled = true), 800);
	}

	function flowing() {
		clearTimeout(stall);
		stalled = false;
	}

	// Autoplay can be refused (Low Power Mode, browser policy). The video then
	// stays paused and the controls stay up with the play button showing.
	async function play() {
		try {
			await video.play();
		} catch {
			paused = true;
			ready = true;
		}
	}

	function toggle() {
		if (video.paused) play();
		else video.pause();
		wake();
	}

	// Controls show on any movement and fade after a still moment, taking the
	// cursor with them; paused, they stay.
	function wake(hold = IDLE_MS) {
		idle = false;
		clearTimeout(timer);
		timer = setTimeout(() => (idle = true), hold);
	}

	// The idle clock starts when the picture arrives, not when the page does:
	// on a slow load it would otherwise run out while still on black.
	$effect(() => {
		if (ready) wake(INTRO_MS);
	});

	// A tap on the picture shows the controls, or hides them if they are up.
	function tap(event: MouseEvent) {
		if (event.target !== main && event.target !== video) return;
		if (idle) wake();
		else {
			clearTimeout(timer);
			idle = true;
		}
	}

	/** Seconds as m:ss, the way a player shows them. */
	function clock(seconds: number) {
		const whole = Math.max(0, Math.round(seconds));
		return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`;
	}

	/** Where a pointer sits along the track, 0 to 1. */
	function along(event: PointerEvent) {
		const box = track.getBoundingClientRect();
		return Math.min(Math.max((event.clientX - box.left) / box.width, 0), 1);
	}

	function seekTo(at: number, exact: boolean) {
		progress = at;
		const t = at * video.duration;
		// While dragging, jump to the nearest keyframe where the browser can,
		// so the picture keeps up with the pointer; land exactly on release.
		if (!exact && video.fastSeek) video.fastSeek(t);
		else video.currentTime = t;
	}

	function scrubStart(event: PointerEvent) {
		if (!video.duration || event.button > 0) return;
		event.preventDefault();
		track.setPointerCapture(event.pointerId);
		scrubbing = true;
		resumeAfterScrub = !video.paused;
		video.pause();
		seekTo(along(event), false);
		wake();
	}

	function scrubMove(event: PointerEvent) {
		if (scrubbing) {
			seekTo(along(event), false);
			wake();
		} else if (event.pointerType === 'mouse') {
			hoverAt = along(event);
		}
	}

	function scrubEnd(event: PointerEvent) {
		if (!scrubbing) return;
		scrubbing = false;
		seekTo(along(event), true);
		if (resumeAfterScrub) play();
		wake();
	}

	/** Step through the loop, wrapping at either end. */
	function skip(by: number) {
		if (!video.duration) return;
		video.currentTime = (video.currentTime + by + video.duration) % video.duration;
		wake();
	}

	function remember() {
		try {
			localStorage.setItem(VOLUME_KEY, String(volume));
			localStorage.setItem(SOUND_KEY, sound ? '1' : '0');
		} catch {
			// Not remembering is fine; the page works the same without storage.
		}
	}

	/** Slider position to loudness: we hear loudness roughly as the square. */
	const gainOf = (v: number) => v * v;

	/** Glide the film's volume to `to` over `seconds`, then run `done`. */
	function glide(to: number, seconds: number, done?: () => void) {
		cancelAnimationFrame(ramp);
		const from = video.volume;
		const start = performance.now();
		ramp = requestAnimationFrame(function step(now) {
			const k = Math.min((now - start) / (seconds * 1000), 1);
			video.volume = from + (to - from) * k * k * (3 - 2 * k);
			if (k < 1) ramp = requestAnimationFrame(step);
			else done?.();
		});
	}

	function startSound() {
		sound = true;
		if (volume === 0) volume = 0.5;
		remember();
		// Sound swells in rather than cutting in.
		video.volume = 0;
		video.muted = false;
		glide(gainOf(volume), 0.6);
	}

	function stopSound() {
		sound = false;
		remember();
		glide(0, 0.3, () => (video.muted = true));
	}

	function toggleSound() {
		wake();
		if (sound) stopSound();
		else startSound();
	}

	function onVolume(event: Event) {
		volume = Number((event.currentTarget as HTMLInputElement).value);
		wake();
		if (volume === 0) {
			if (sound) stopSound();
		} else if (!sound) startSound();
		else {
			cancelAnimationFrame(ramp);
			video.volume = gainOf(volume);
			remember();
		}
	}

	async function toggleFullscreen() {
		wake();
		if (document.fullscreenElement) {
			await document.exitFullscreen();
			return;
		}
		if (main.requestFullscreen) {
			await main.requestFullscreen({ navigationUI: 'hide' });
			// Phones on Android can be held to landscape once fullscreen; desktops
			// and tablets refuse the lock, and the picture stays as it is.
			const orientation = screen.orientation as ScreenOrientation & {
				lock?: (o: string) => Promise<void>;
			};
			await orientation.lock?.('landscape').catch(() => {});
		} else {
			// iPhone Safari has no element fullscreen: hand over to the native
			// player, which turns with the phone.
			video.webkitEnterFullscreen?.();
		}
	}

	function key(event: KeyboardEvent) {
		// The volume slider takes its own arrow keys.
		const onSlider = (event.target as HTMLElement).matches?.('input[type="range"]');
		if (event.key === ' ' || event.key === 'k') {
			event.preventDefault();
			toggle();
		} else if ((event.key === 'ArrowRight' || event.key === 'ArrowLeft') && !onSlider) {
			event.preventDefault();
			skip(event.key === 'ArrowRight' ? 5 : -5);
		} else if (event.key === 'f') {
			toggleFullscreen();
		} else if (event.key === 'm') {
			toggleSound();
		} else {
			wake();
		}
	}

	const shown = $derived(ready && (!idle || paused));
</script>

<svelte:head>
	<title>Coalesce</title>
	<meta
		name="description"
		content="Two black holes spiral together and become one, scored to its song. A film by Oddur Sigurdsson."
	/>
	<meta name="theme-color" content="#000000" />
</svelte:head>

<svelte:window onkeydown={key} />

<main
	bind:this={main}
	class:idle={!shown}
	onclick={tap}
	onpointermove={(e) => e.pointerType === 'mouse' && wake()}
	role="presentation"
	style="--hero: url({media('hero.jpg')})"
>
	<video
		bind:this={video}
		class:ready
		autoplay
		muted
		loop
		playsinline
		disablepictureinpicture
		poster={media('poster.jpg')}
		onloadeddata={play}
		onplay={() => (paused = false)}
		onplaying={() => {
			ready = true;
			flowing();
		}}
		onwaiting={waiting}
		onerror={() => (ready = true)}
		onpause={() => {
			paused = true;
			flowing();
		}}
	>
		{#each sources as [src, type, query], i (i)}
			<!-- The last source failing means none could play: show the poster
			     rather than wait on black. -->
			<source
				{src}
				{type}
				media={query}
				onerror={i === sources.length - 1 ? () => (ready = true) : undefined}
			/>
		{/each}
	</video>

	<div class="loader" class:done={ready && !stalled} aria-hidden="true"><span></span></div>

	<p class="credit" class:shown>
		Coalesce <span>·</span>
		<a href="https://github.com/oddurs" rel="me">Oddur Sigurdsson</a>
	</p>

	<p class="turn" aria-hidden="true">
		<svg viewBox="0 0 24 24" width="28" height="28">
			<rect x="7" y="3" width="10" height="18" rx="2.2" />
			<line x1="10.5" y1="18" x2="13.5" y2="18" />
		</svg>
		Turn your phone sideways
	</p>

	<div class="controls" class:shown class:scrubbing>
		<button class="play" onclick={toggle} aria-label={paused ? 'Play' : 'Pause'}>
			{#if paused}
				<svg viewBox="0 0 24 24"
					><path
						d="M7.5 5.2v13.6c0 .9 1 1.4 1.7.9l10.3-6.8c.7-.4.7-1.4 0-1.8L9.2 4.3c-.7-.5-1.7 0-1.7.9z"
					/></svg
				>
			{:else}
				<svg viewBox="0 0 24 24"
					><rect x="6" y="4.5" width="4.2" height="15" rx="1.3" /><rect
						x="13.8"
						y="4.5"
						width="4.2"
						height="15"
						rx="1.3"
					/></svg
				>
			{/if}
		</button>
		<span class="time">{clock(progress * duration)}</span>
		<div
			class="track"
			bind:this={track}
			onpointerdown={scrubStart}
			onpointermove={scrubMove}
			onpointerup={scrubEnd}
			onpointercancel={scrubEnd}
			onpointerleave={() => (hoverAt = null)}
			role="slider"
			tabindex="0"
			aria-label="Position"
			aria-valuemin={0}
			aria-valuemax={Math.round(duration)}
			aria-valuenow={Math.round(progress * duration)}
			aria-valuetext="{clock(progress * duration)} of {clock(duration)}"
		>
			<div class="rail">
				<div class="buffered" style:transform="scaleX({buffered})"></div>
				<div class="fill" style:transform="scaleX({progress})"></div>
			</div>
			<div class="knob" style:left="{progress * 100}%"></div>
			{#if scrubbing || hoverAt !== null}
				<div class="tip" style:--at={scrubbing ? progress : hoverAt}>
					{clock((scrubbing ? progress : (hoverAt ?? 0)) * duration)}
				</div>
			{/if}
		</div>
		<span class="time remaining">&minus;{clock(duration - progress * duration)}</span>
		<div class="volume" class:on={sound} class:fixed={fixedVolume}>
			<button
				onclick={toggleSound}
				aria-label={sound ? 'Mute' : 'Sound on'}
				aria-pressed={sound}
			>
				<svg viewBox="0 0 24 24">
					<path
						d="M3.5 9.3c0-.6.4-1 1-1h3l4.3-3.6c.6-.5 1.5-.1 1.5.7v13.2c0 .8-.9 1.2-1.5.7L7.5 15.7h-3c-.6 0-1-.4-1-1z"
					/>
					{#if sound}
						<path
							class="wave"
							d="M16 9.2a4.2 4.2 0 0 1 0 5.6M18.6 6.6a8 8 0 0 1 0 10.8"
						/>
					{:else}
						<path class="wave" d="M16.3 9.8l4.4 4.4M20.7 9.8l-4.4 4.4" />
					{/if}
				</svg>
			</button>
			<input
				type="range"
				min="0"
				max="1"
				step="0.01"
				value={sound ? volume : 0}
				oninput={onVolume}
				aria-label="Volume"
				style:--level={sound ? volume : 0}
			/>
		</div>
		<button
			onclick={toggleFullscreen}
			aria-label={fullscreen ? 'Exit full screen' : 'Full screen'}
		>
			<svg viewBox="0 0 24 24" class="stroke">
				{#if fullscreen}
					<path d="M9.5 4.5v5h-5M14.5 4.5v5h5M9.5 19.5v-5h-5M14.5 19.5v-5h5" />
				{:else}
					<path d="M4.5 9.5v-5h5M19.5 9.5v-5h-5M4.5 14.5v5h5M19.5 14.5v5h-5" />
				{/if}
			</svg>
		</button>
	</div>
</main>

<style>
	:global(html, body) {
		margin: 0;
		height: 100%;
		background: #000;
		overflow: hidden;
	}
	main {
		position: fixed;
		inset: 0;
		background: #000;
		font-family: -apple-system, BlinkMacSystemFont, system-ui, sans-serif;
		-webkit-tap-highlight-color: transparent;
	}
	main.idle {
		cursor: none;
	}
	video {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
		opacity: 0;
		transition: opacity 1.6s ease;
	}
	video.ready {
		opacity: 1;
	}

	/* While the film loads: one breathing point of light, then it fades away
	   as the picture fades up. */
	.loader {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		pointer-events: none;
		transition: opacity 0.6s ease;
	}
	.loader.done {
		opacity: 0;
	}
	.loader span {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: rgba(255, 255, 255, 0.8);
		box-shadow: 0 0 14px rgba(255, 255, 255, 0.35);
		animation: breathe 1.8s ease-in-out infinite;
	}
	@keyframes breathe {
		0%,
		100% {
			transform: scale(0.7);
			opacity: 0.3;
		}
		50% {
			transform: scale(1.15);
			opacity: 0.9;
		}
	}

	/* The title and credit ride with the controls: there when you reach for
	   them, gone while you watch. */
	.credit {
		position: absolute;
		top: max(1.25rem, env(safe-area-inset-top));
		left: max(1.5rem, env(safe-area-inset-left));
		margin: 0;
		font-size: 0.8rem;
		letter-spacing: 0.03em;
		color: rgba(255, 255, 255, 0.85);
		opacity: 0;
		pointer-events: none;
		transition: opacity 0.35s ease;
	}
	.credit.shown {
		opacity: 1;
		pointer-events: auto;
	}
	.credit span {
		margin: 0 0.3em;
		color: rgba(255, 255, 255, 0.4);
	}
	.credit a {
		color: rgba(255, 255, 255, 0.55);
		text-decoration: none;
		transition: color 0.2s ease;
	}
	.credit a:hover,
	.credit a:focus-visible {
		color: #fff;
	}
	.credit a:focus-visible {
		outline: 2px solid rgba(255, 255, 255, 0.8);
		outline-offset: 3px;
		border-radius: 2px;
	}

	/* The player, after Apple's: a floating glass bar, its buttons and the
	   scrubber sized for a finger on touch screens and for a cursor elsewhere. */
	.controls {
		position: absolute;
		left: 50%;
		bottom: max(1.25rem, env(safe-area-inset-bottom));
		transform: translate(-50%, 0.5rem);
		width: min(
			40rem,
			calc(100% - 2rem - env(safe-area-inset-left) - env(safe-area-inset-right))
		);
		box-sizing: border-box;
		display: flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.3rem 0.5rem;
		border-radius: 1.25rem;
		background: rgba(30, 30, 32, 0.5);
		backdrop-filter: blur(30px) saturate(180%);
		-webkit-backdrop-filter: blur(30px) saturate(180%);
		box-shadow:
			inset 0 0 0 0.5px rgba(255, 255, 255, 0.14),
			0 10px 30px rgba(0, 0, 0, 0.35);
		font:
			500 0.75rem/1 -apple-system,
			BlinkMacSystemFont,
			'SF Pro Text',
			system-ui,
			sans-serif;
		font-variant-numeric: tabular-nums;
		letter-spacing: 0.01em;
		color: rgba(255, 255, 255, 0.72);
		opacity: 0;
		pointer-events: none;
		transition:
			opacity 0.3s ease,
			transform 0.3s cubic-bezier(0.2, 0.8, 0.2, 1);
		user-select: none;
		-webkit-user-select: none;
	}
	.controls.shown {
		opacity: 1;
		transform: translate(-50%, 0);
		pointer-events: auto;
	}
	button {
		flex: none;
		display: grid;
		place-items: center;
		width: 2.5rem;
		height: 2.5rem;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: transparent;
		color: #fff;
		cursor: pointer;
		-webkit-tap-highlight-color: transparent;
		transition:
			background 0.15s ease,
			transform 0.12s ease;
	}
	button:hover {
		background: rgba(255, 255, 255, 0.12);
	}
	button:active {
		transform: scale(0.9);
	}
	button:focus-visible,
	.track:focus-visible {
		outline: 2px solid rgba(255, 255, 255, 0.85);
		outline-offset: 1px;
	}
	svg {
		width: 1.3rem;
		height: 1.3rem;
		fill: currentColor;
	}
	.play svg {
		width: 1.45rem;
		height: 1.45rem;
	}
	svg .wave,
	svg.stroke {
		fill: none;
		stroke: currentColor;
		stroke-width: 1.8;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	.time {
		flex: none;
		min-width: 2.4rem;
		text-align: center;
	}

	/* The scrubber: a tall hit area round a hairline that thickens under the
	   pointer, a knob that appears when reached for, and the time it would
	   jump to floating above. */
	.track {
		flex: 1;
		position: relative;
		align-self: stretch;
		min-width: 3rem;
		margin: 0 0.35rem;
		cursor: pointer;
		touch-action: none;
		-webkit-tap-highlight-color: transparent;
	}
	.rail {
		position: absolute;
		left: 0;
		right: 0;
		top: 50%;
		height: 4px;
		transform: translateY(-50%);
		border-radius: 999px;
		overflow: hidden;
		background: rgba(255, 255, 255, 0.22);
		transition: height 0.15s ease;
	}
	.track:hover .rail,
	.scrubbing .rail {
		height: 6px;
	}
	.buffered,
	.fill {
		position: absolute;
		inset: 0;
		transform-origin: left;
	}
	.buffered {
		background: rgba(255, 255, 255, 0.2);
	}
	.fill {
		background: #fff;
	}
	.knob {
		position: absolute;
		top: 50%;
		width: 14px;
		height: 14px;
		margin: -7px 0 0 -7px;
		border-radius: 50%;
		background: #fff;
		box-shadow: 0 1px 4px rgba(0, 0, 0, 0.4);
		transform: scale(0);
		transition: transform 0.15s ease;
		pointer-events: none;
	}
	.track:hover .knob,
	.scrubbing .knob {
		transform: scale(1);
	}
	.scrubbing .knob {
		transform: scale(1.25);
	}
	.tip {
		position: absolute;
		bottom: calc(50% + 14px);
		left: clamp(1.4rem, calc(var(--at) * 100%), calc(100% - 1.4rem));
		transform: translateX(-50%);
		padding: 0.3rem 0.45rem;
		border-radius: 0.45rem;
		background: rgba(30, 30, 32, 0.8);
		backdrop-filter: blur(20px);
		-webkit-backdrop-filter: blur(20px);
		color: #fff;
		white-space: nowrap;
		pointer-events: none;
	}

	/* Volume: the slider slides out beside the speaker on hover or focus, and
	   stays out on touch screens, where there is no hover. */
	.volume {
		flex: none;
		display: flex;
		align-items: center;
	}
	.volume input {
		width: 0;
		opacity: 0;
		margin: 0;
		transition:
			width 0.3s ease,
			opacity 0.3s ease,
			margin 0.3s ease;
	}
	.volume:hover input,
	.volume:focus-within input {
		width: 4.5rem;
		opacity: 1;
		margin-right: 0.4rem;
	}
	.volume.fixed input {
		display: none;
	}
	input[type='range'] {
		-webkit-appearance: none;
		appearance: none;
		height: 2.5rem;
		background: transparent;
		cursor: pointer;
	}
	input[type='range']::-webkit-slider-runnable-track {
		height: 4px;
		border-radius: 999px;
		background: linear-gradient(
			to right,
			#fff calc(var(--level) * 100%),
			rgba(255, 255, 255, 0.22) 0
		);
	}
	input[type='range']::-webkit-slider-thumb {
		-webkit-appearance: none;
		width: 14px;
		height: 14px;
		margin-top: -5px;
		border-radius: 50%;
		background: #fff;
		box-shadow: 0 1px 4px rgba(0, 0, 0, 0.4);
	}
	input[type='range']::-moz-range-track {
		height: 4px;
		border-radius: 999px;
		background: rgba(255, 255, 255, 0.22);
	}
	input[type='range']::-moz-range-progress {
		height: 4px;
		border-radius: 999px;
		background: #fff;
	}
	input[type='range']::-moz-range-thumb {
		width: 14px;
		height: 14px;
		border: 0;
		border-radius: 50%;
		background: #fff;
	}
	input[type='range']:focus-visible {
		outline: 2px solid rgba(255, 255, 255, 0.85);
		outline-offset: 2px;
		border-radius: 4px;
	}

	/* Touch screens: 44 px targets, the knob always there to grab, the volume
	   slider out, and no double-tap zoom. */
	@media (hover: none) {
		main {
			touch-action: manipulation;
		}
		button {
			width: 2.75rem;
			height: 2.75rem;
		}
		button:hover {
			background: transparent;
		}
		.knob {
			transform: scale(0.85);
		}
		.volume input {
			width: 4rem;
			opacity: 1;
			margin-right: 0.3rem;
		}
	}
	/* Narrow screens keep the bar calm: one time, counting down. */
	@media (max-width: 560px) {
		.time:not(.remaining) {
			display: none;
		}
		.volume input {
			display: none;
		}
	}

	/* Upright phones: the film is 1.90:1, so show the whole frame and ask for
	   the phone to be turned. Sideways it fills the screen. */
	.turn {
		display: none;
	}
	@media (orientation: portrait) and (hover: none) {
		video {
			object-fit: contain;
		}
		.turn {
			position: absolute;
			left: 0;
			right: 0;
			/* Just below the letterboxed frame, which is 100vw / 1.9 tall. */
			top: calc(50% + 50vw / 1.9 + 1.5rem);
			margin: 0;
			display: flex;
			flex-direction: column;
			align-items: center;
			gap: 0.6rem;
			color: rgba(255, 255, 255, 0.55);
			font-size: 0.8rem;
			letter-spacing: 0.02em;
		}
		.turn svg {
			width: 1.75rem;
			height: 1.75rem;
			fill: none;
			stroke: currentColor;
			stroke-width: 1.4;
			stroke-linecap: round;
			animation: turn 3.2s ease-in-out infinite;
		}
	}
	@keyframes turn {
		0%,
		30% {
			transform: rotate(0deg);
		}
		55%,
		85% {
			transform: rotate(-90deg);
		}
		100% {
			transform: rotate(0deg);
		}
	}

	/* No motion: the loop gives way to one still from the middle of it, not
	   the opening frame, which is empty sky. */
	@media (prefers-reduced-motion: reduce) {
		main {
			background: #000 var(--hero) center / cover no-repeat;
		}
		video,
		.controls,
		.loader,
		.turn {
			display: none;
		}
		.credit {
			opacity: 1;
			pointer-events: auto;
		}
	}
</style>
