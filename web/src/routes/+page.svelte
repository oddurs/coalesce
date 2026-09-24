<script lang="ts">
	import { env } from '$env/dynamic/public';

	// Encoded loops live outside the repo; the base URL points at wherever
	// scripts/encode output was uploaded. Locally it is /video under static/.
	const base = env.PUBLIC_VIDEO_BASE || '/video';
	const sources = [
		['loop-4k-av1.mp4', 'video/mp4; codecs="av01.0.12M.10"'],
		['loop-4k.mp4', 'video/mp4; codecs="hvc1.2.4.L153.B0"'],
		['loop-1080.mp4', 'video/mp4; codecs="avc1.640028"']
	];
	let video: HTMLVideoElement;
	let ready = $state(false);
</script>

<svelte:head>
	<title>Merger</title>
	<meta
		name="description"
		content="Two black holes spiral together and become one. A sixty second loop."
	/>
</svelte:head>

<main class:ready>
	<video
		bind:this={video}
		autoplay
		muted
		loop
		playsinline
		disablepictureinpicture
		poster="{base}/poster.jpg"
		oncanplay={() => (ready = true)}
	>
		{#each sources as [file, type] (file)}
			<source src="{base}/{file}" {type} />
		{/each}
	</video>
	<img src="{base}/poster.jpg" alt="Two black holes lensing the light of an accretion disk" />
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
	video,
	img {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
	video {
		opacity: 0;
		transition: opacity 1.2s ease;
	}
	main.ready video {
		opacity: 1;
	}
	@media (prefers-reduced-motion: reduce) {
		video {
			display: none;
		}
	}
</style>
