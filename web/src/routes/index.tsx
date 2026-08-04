import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/")({
	component: Home,
});

function Home() {
	return (
		<main className="page-wrap flex min-h-[70vh] flex-col items-center justify-center px-4 py-12 text-center">
			<div className="island-shell relative overflow-hidden rounded-3xl p-8 sm:p-12 md:p-16 max-w-2xl w-full transition-all duration-300 hover:shadow-xl dark:shadow-[0_0_50px_rgba(255,255,255,0.02)]">
				<div className="absolute top-0 left-0 right-0 h-1 bg-gradient-to-r from-teal-400 via-emerald-400 to-cyan-400" />

				<div className="mb-6 inline-flex h-12 w-12 items-center justify-center rounded-2xl bg-[var(--chip-bg)] border border-[var(--chip-line)] text-2xl animate-bounce">
					✨
				</div>

				<h1 className="mb-4 text-4xl font-extrabold tracking-tight text-[var(--sea-ink)] sm:text-5xl md:text-6xl bg-gradient-to-r from-[var(--sea-ink)] via-[var(--sea-ink-soft)] to-[var(--sea-ink)] bg-clip-text text-transparent">
					Civic Sync
				</h1>

				<p className="mx-auto mb-8 max-w-md text-base leading-relaxed text-[var(--sea-ink-soft)]">
					A clean, minimalistic React application powered by TanStack Start. All
					extra routes and complexities have been removed.
				</p>

				<div className="flex flex-col sm:flex-row items-center justify-center gap-4">
					<button
						type="button"
						onClick={() => alert("Welcome to Civic Sync!")}
						className="w-full sm:w-auto px-6 py-3 text-sm font-semibold text-white bg-black dark:bg-white dark:text-black rounded-xl shadow-lg hover:opacity-90 active:scale-95 transition-all cursor-pointer"
					>
						Get Started
					</button>
					<a
						href="https://tanstack.com/router/v1"
						target="_blank"
						rel="noreferrer"
						className="w-full sm:w-auto px-6 py-3 text-sm font-semibold border border-[var(--line)] rounded-xl hover:bg-[var(--chip-bg)] active:scale-95 transition-all text-[var(--sea-ink)]"
					>
						Documentation
					</a>
				</div>
			</div>
		</main>
	);
}
