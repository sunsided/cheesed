<!--
  Embeds the Cheesed! wasm build in a SvelteKit page (Svelte 5).

  Copy the release build into your app's static folder first:
    task web:export DEST=../my-site/static/cheesed

  Then:
    <Cheesed onexit={() => goto('/games')} oncomplete={(r) => showStars(r.stars, r.max)} />

  The game binds to its <canvas> once and has no teardown, so the canvas and the wasm module
  live for the whole page session. Unmounting this component only detaches the canvas (the
  game pauses and goes silent while detached); mounting it again re-attaches the same canvas
  and resumes, or, with `fresh`, starts over from the title. Starting over keeps the sound on
  once the player has unlocked it, and saved progress is kept either way.

  `restart()` (exported from this module) starts over at any time.
-->
<script module lang="ts">
	export interface Complete {
		/** Stars earned over all levels of the episode. */
		stars: number;
		/** The most stars there are (three per level). */
		max: number;
	}

	let canvas: HTMLCanvasElement | null = null;
	let started: Promise<void> | null = null;

	function getCanvas(): HTMLCanvasElement {
		if (!canvas) {
			canvas = document.createElement('canvas');
			canvas.id = 'cheese';
			canvas.tabIndex = 0;
			canvas.setAttribute('aria-label', 'Cheesed! action adventure game');
		}
		return canvas;
	}

	/** Start over at the title as if freshly launched (saved progress is kept). */
	export function restart(): void {
		canvas?.dispatchEvent(new CustomEvent('cheese:restart'));
	}

	// `ref.null func; drop`: valid only with WebAssembly reference types, which the build uses
	// (like every Rust wasm build since 1.82). There is no polyfill for missing wasm features.
	const REF_TYPES = new Uint8Array([
		0, 97, 115, 109, 1, 0, 0, 0, 1, 4, 1, 96, 0, 0, 3, 2, 1, 0, 10, 7, 1, 5, 0, 208, 112, 26, 11
	]);

	/** Why this browser can't run the game, or `null` if it can. */
	function unsupported(): string | null {
		if (typeof WebAssembly !== 'object' || !WebAssembly.validate(REF_TYPES)) {
			return 'This browser is too old for the game. It runs in Chrome 96, Firefox 79, Safari 15 or newer.';
		}
		if (!('gpu' in navigator)) {
			const gl = document.createElement('canvas').getContext('webgl2');
			if (!gl) return 'The game needs WebGPU or WebGL2, which this browser has turned off or lacks.';
			gl.getExtension('WEBGL_lose_context')?.loseContext();
		}
		return null;
	}

	function start(base: string): Promise<void> {
		started ??= import(/* @vite-ignore */ `${base}/cheese.js`).then((m) => m.default());
		return started;
	}
</script>

<script lang="ts">
	interface Props {
		/** URL path the build was exported to (no trailing slash). */
		base?: string;
		/** Focus the game on mount so the keyboard works right away. */
		autofocus?: boolean;
		/** Start over when mounted again, instead of resuming where the player left. */
		fresh?: boolean;
		/** The player left (Esc in the level menu, then the farewell); the game has gone quiet. */
		onexit?: () => void;
		/** A level started (1-based). */
		onstart?: (level: number) => void;
		/** The player reached the end of the episode. */
		oncomplete?: (result: Complete) => void;
	}

	let {
		base = '/cheesed',
		autofocus = false,
		fresh = false,
		onexit,
		onstart,
		oncomplete
	}: Props = $props();

	let error = $state<string | null>(null);

	/** Attach the shared canvas to `slot`, start the game once, detach on unmount. */
	function mount(slot: HTMLElement) {
		const why = unsupported();
		if (why) {
			error = why;
			return;
		}
		const c = getCanvas();
		const exit = () => onexit?.();
		const begin = (e: Event) => onstart?.((e as CustomEvent<{ level: number }>).detail.level);
		const done = (e: Event) => oncomplete?.((e as CustomEvent<Complete>).detail);
		c.addEventListener('cheese:exit', exit);
		c.addEventListener('cheese:start', begin);
		c.addEventListener('cheese:complete', done);
		const again = started !== null;
		slot.appendChild(c);
		if (autofocus) c.focus();
		if (again && fresh) restart();
		// The canvas must be in the document before the game starts, as it looks it up by id.
		start(base).catch((e) => {
			// winit throws a control-flow exception once the event loop is running; that is expected.
			if (!String(e).includes('Using exceptions for control flow')) {
				error = `Could not start the game: ${e}`;
				started = null;
			}
		});
		return () => {
			c.removeEventListener('cheese:exit', exit);
			c.removeEventListener('cheese:start', begin);
			c.removeEventListener('cheese:complete', done);
			c.remove();
		};
	}
</script>

<div class="cheesed">
	<div class="slot" {@attach mount}></div>
	{#if error}
		<p class="error">{error}</p>
	{/if}
</div>

<style>
	.cheesed {
		position: relative;
		width: 100%;
		max-width: 1280px;
		/* 320x200: the shape of the game's screen; it letterboxes inside. */
		aspect-ratio: 16 / 10;
		background: #000;
	}
	.slot {
		position: absolute;
		inset: 0;
	}
	.slot :global(canvas#cheese) {
		display: block;
		/* winit sets an inline size; the container decides instead. */
		width: 100% !important;
		height: 100% !important;
		outline: none;
		touch-action: none;
		user-select: none;
		-webkit-user-select: none;
		-webkit-touch-callout: none;
		image-rendering: pixelated;
	}
	.error {
		position: absolute;
		inset: 0;
		margin: 0;
		display: grid;
		place-items: center;
		color: #f06a6a;
		font: 14px system-ui, sans-serif;
	}
</style>
