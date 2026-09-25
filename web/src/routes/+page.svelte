<script lang="ts">
	import { onMount } from 'svelte';
	import { env } from '$env/dynamic/public';

	// Encoded loops live outside the repo; the base URL points at wherever
	// scripts/encode output was uploaded. Locally it is /video under static/.
	const base = env.PUBLIC_VIDEO_BASE || '/video';
	const candidates = [
		['loop-4k-av1.mp4', 'video/mp4; codecs="av01.0.12M.10"'],
		['loop-4k.mp4', 'video/mp4; codecs="hvc1.2.4.L153.B0"'],
		['loop-1080.mp4', 'video/mp4; codecs="avc1.640028"']
	];

	let video: HTMLVideoElement;
	let src = $state('');
	let blocked = $state(false);

	// Pick the first encode the browser can decode and the server actually has,
	// so a missing 4K file never leaves the page on its poster.
	onMount(async () => {
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
			break;
		}
	});

	async function play() {
		try {
			await video.play();
			blocked = false;
		} catch {
			blocked = true;
		}
	}
</script>

<svelte:head>
	<title>Merger</title>
	<meta
		name="description"
		content="Two black holes spiral together and become one. A sixty second loop."
	/>
</svelte:head>

<main onclick={play} role="presentation" style="--hero: url({base}/hero.jpg)">
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
	></video>
	{#if blocked}
		<button onclick={play} aria-label="Play">Play</button>
	{/if}
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
	}
	video {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
	button {
		position: absolute;
		left: 50%;
		top: 50%;
		transform: translate(-50%, -50%);
		padding: 0.8em 1.6em;
		font:
			500 1rem system-ui,
			sans-serif;
		color: #fff;
		background: rgba(0, 0, 0, 0.5);
		border: 1px solid rgba(255, 255, 255, 0.5);
		border-radius: 999px;
		cursor: pointer;
	}
	/* No motion: the loop gives way to one still from the middle of it, not
	   the opening frame, which is empty sky. */
	@media (prefers-reduced-motion: reduce) {
		main {
			background: #000 var(--hero) center / cover no-repeat;
		}
		video {
			display: none;
		}
	}
</style>
