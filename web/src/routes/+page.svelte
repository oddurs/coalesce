<script lang="ts">
	import { onMount } from 'svelte';

	// The web encodes from scripts/encode ship with the site under /video.
	const base = '/video';
	const small = ['loop-720.mp4', 'video/mp4; codecs="avc1.64001f"'];
	const full = [
		['loop-av1.mp4', 'video/mp4; codecs="av01.0.08M.10"'],
		['loop-hevc.mp4', 'video/mp4; codecs="hvc1.2.4.L120.B0"'],
		small
	];
	const IDLE_MS = 2500;

	type IosVideo = HTMLVideoElement & { webkitEnterFullscreen?: () => void };

	let main: HTMLElement;
	let video: IosVideo;
	let src = $state('');
	let paused = $state(true);
	let idle = $state(false);
	let fullscreen = $state(false);
	let progress = $state(0);
	let timer: ReturnType<typeof setTimeout> | undefined;

	// Phones and Data Saver get the small encode: full 2K is wasted on a
	// small screen and costs the most bandwidth. Judged by the screen's short
	// side, since a large phone held sideways is wider than many laptops.
	// Otherwise pick the first encode the browser can decode and the server has.
	onMount(() => {
		const saveData = (navigator as { connection?: { saveData?: boolean } }).connection
			?.saveData;
		const phone = Math.min(screen.width, screen.height) <= 600;
		const candidates = phone || saveData ? [small] : full;
		pick(candidates);

		const onFullscreen = () => (fullscreen = document.fullscreenElement !== null);
		document.addEventListener('fullscreenchange', onFullscreen);
		let frame = requestAnimationFrame(function tick() {
			if (video.duration) progress = video.currentTime / video.duration;
			frame = requestAnimationFrame(tick);
		});
		wake();
		return () => {
			document.removeEventListener('fullscreenchange', onFullscreen);
			cancelAnimationFrame(frame);
			clearTimeout(timer);
		};
	});

	async function pick(candidates: string[][]) {
		for (const [file, type] of candidates) {
			if (!video.canPlayType(type)) continue;
			const url = `${base}/${file}`;
			try {
				const head = await fetch(url, { method: 'HEAD' });
				if (!head.ok) continue;
			} catch {
				continue;
			}
			src = url;
			return;
		}
	}

	// Autoplay can be refused (Low Power Mode, browser policy). The video then
	// stays paused and the controls stay up with the play button showing.
	async function play() {
		try {
			await video.play();
		} catch {
			paused = true;
		}
	}

	function toggle() {
		if (video.paused) play();
		else video.pause();
		wake();
	}

	// Controls show on any movement and fade after a still moment, taking the
	// cursor with them; paused, they stay.
	function wake() {
		idle = false;
		clearTimeout(timer);
		timer = setTimeout(() => (idle = true), IDLE_MS);
	}

	// A tap on the picture shows the controls, or hides them if they are up.
	function tap(event: MouseEvent) {
		if (event.target !== main && event.target !== video) return;
		if (idle) wake();
		else {
			clearTimeout(timer);
			idle = true;
		}
	}

	function seek(event: MouseEvent) {
		const track = event.currentTarget as HTMLElement;
		const box = track.getBoundingClientRect();
		const at = Math.min(Math.max((event.clientX - box.left) / box.width, 0), 1);
		if (video.duration) video.currentTime = at * video.duration;
		wake();
	}

	// Arrow keys on the track step through the loop, five seconds at a time.
	function step(event: KeyboardEvent) {
		const by = event.key === 'ArrowRight' ? 5 : event.key === 'ArrowLeft' ? -5 : 0;
		if (!by || !video.duration) return;
		event.preventDefault();
		event.stopPropagation();
		video.currentTime = (video.currentTime + by + video.duration) % video.duration;
		wake();
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
		if (event.key === ' ' || event.key === 'k') {
			event.preventDefault();
			toggle();
		} else if (event.key === 'f') {
			toggleFullscreen();
		} else {
			wake();
		}
	}

	const shown = $derived(!idle || paused);
</script>

<svelte:head>
	<title>Merger</title>
	<meta
		name="description"
		content="Two black holes spiral together and become one. A sixty second loop."
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
	style="--hero: url({base}/hero.jpg)"
>
	<video
		bind:this={video}
		{src}
		autoplay
		muted
		loop
		playsinline
		disablepictureinpicture
		poster="{base}/poster.jpg"
		onloadeddata={play}
		onplay={() => (paused = false)}
		onpause={() => (paused = true)}
	></video>

	<p class="turn" aria-hidden="true">
		<svg viewBox="0 0 24 24" width="28" height="28">
			<rect x="7" y="3" width="10" height="18" rx="2.2" />
			<line x1="10.5" y1="18" x2="13.5" y2="18" />
		</svg>
		Turn your phone sideways
	</p>

	<div class="controls" class:shown>
		<button onclick={toggle} aria-label={paused ? 'Play' : 'Pause'}>
			{#if paused}
				<svg viewBox="0 0 24 24"><path d="M8 5.5v13l10.5-6.5z" /></svg>
			{:else}
				<svg viewBox="0 0 24 24"
					><rect x="7" y="5.5" width="3.4" height="13" rx="1" /><rect
						x="13.6"
						y="5.5"
						width="3.4"
						height="13"
						rx="1"
					/></svg
				>
			{/if}
		</button>
		<div
			class="track"
			onclick={seek}
			onkeydown={step}
			role="slider"
			tabindex="0"
			aria-label="Position in the loop"
			aria-valuemin={0}
			aria-valuemax={100}
			aria-valuenow={Math.round(progress * 100)}
		>
			<div class="fill" style:transform="scaleX({progress})"></div>
		</div>
		<button
			onclick={toggleFullscreen}
			aria-label={fullscreen ? 'Exit full screen' : 'Full screen'}
		>
			{#if fullscreen}
				<svg viewBox="0 0 24 24" class="stroke"
					><path d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" /></svg
				>
			{:else}
				<svg viewBox="0 0 24 24" class="stroke"
					><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" /></svg
				>
			{/if}
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
	}

	.controls {
		position: absolute;
		left: 50%;
		bottom: max(1.5rem, env(safe-area-inset-bottom));
		transform: translate(-50%, 0.5rem);
		width: min(
			28rem,
			calc(100% - 2rem - env(safe-area-inset-left) - env(safe-area-inset-right))
		);
		box-sizing: border-box;
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.4rem 0.55rem;
		border-radius: 999px;
		background: rgba(28, 28, 30, 0.45);
		backdrop-filter: blur(24px) saturate(160%);
		-webkit-backdrop-filter: blur(24px) saturate(160%);
		border: 0.5px solid rgba(255, 255, 255, 0.12);
		opacity: 0;
		pointer-events: none;
		transition:
			opacity 0.35s ease,
			transform 0.35s ease;
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
		width: 2.25rem;
		height: 2.25rem;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: transparent;
		color: #fff;
		cursor: pointer;
	}
	button:hover {
		background: rgba(255, 255, 255, 0.1);
	}
	button:focus-visible,
	.track:focus-visible {
		outline: 2px solid rgba(255, 255, 255, 0.8);
		outline-offset: 2px;
	}
	svg {
		width: 1.25rem;
		height: 1.25rem;
		fill: currentColor;
	}
	svg.stroke {
		fill: none;
		stroke: currentColor;
		stroke-width: 2;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	/* A tall hit area around a hairline, so it is easy to tap. */
	.track {
		flex: 1;
		position: relative;
		height: 1.75rem;
		cursor: pointer;
	}
	.track::before,
	.fill {
		content: '';
		position: absolute;
		left: 0;
		right: 0;
		top: 50%;
		height: 3px;
		margin-top: -1.5px;
		border-radius: 3px;
	}
	.track::before {
		background: rgba(255, 255, 255, 0.28);
	}
	.fill {
		background: #fff;
		transform-origin: left;
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
		.turn {
			display: none;
		}
	}
</style>
