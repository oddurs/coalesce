import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { defineConfig } from 'vite';

// The media keeps fixed names, so each URL carries a hash of its content:
// browsers can then cache a file for good and still see a new render the
// moment it ships. The files are gitignored, so a checkout may have none.
function versions(...dirs: string[]) {
	const out: Record<string, string> = {};
	for (const dir of dirs.map((d) => `static/${d}`).filter(existsSync)) {
		for (const file of readdirSync(dir).filter((f) => !f.startsWith('.'))) {
			const hash = createHash('sha256').update(readFileSync(`${dir}/${file}`));
			out[`${dir.slice('static'.length)}/${file}`] = hash.digest('hex').slice(0, 12);
		}
	}
	return out;
}

export default defineConfig({
	define: {
		__MEDIA__: JSON.stringify(versions('video', 'audio'))
	},
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},

			// A static build: one prerendered page and the video files, so any
			// static host or CDN can serve it without functions.
			adapter: adapter()
		})
	]
});
