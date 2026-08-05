import { Link } from "@tanstack/react-router";
import ThemeToggle from "./ThemeToggle";

export default function Header() {
	return (
		<header className="sticky top-0 z-50 border-b border-white/10 dark:border-slate-800/50 bg-gradient-to-r from-rose-100 via-slate-50 to-indigo-100 dark:from-rose-950 dark:via-slate-900 dark:to-indigo-950 px-4 backdrop-blur-xl shadow-sm transition-colors duration-300">
			<nav className="max-w-7xl mx-auto flex items-center justify-between py-4">
				<h2 className="m-0 flex-shrink-0 text-base font-bold tracking-tight">
					<Link
						to="/"
						className="inline-flex items-center gap-2 text-slate-900 dark:text-white no-underline transition-transform hover:scale-105"
					>
						<span className="h-3 w-3 rounded-full bg-rose-500 shadow-[0_0_10px_rgba(244,63,94,0.8)] animate-pulse" />
						<span className="font-extrabold uppercase tracking-widest text-sm bg-gradient-to-r from-rose-600 to-indigo-600 dark:from-rose-400 dark:to-indigo-400 bg-clip-text text-transparent">
							Civic Sync
						</span>
					</Link>
				</h2>

				<div className="flex items-center gap-1.5 sm:gap-4">
					<Link
						to="/admin"
						className="rounded-xl px-4 py-2 text-xs font-bold uppercase tracking-widest text-slate-700 dark:text-slate-300 hover:bg-white/50 dark:hover:bg-slate-800/50 hover:text-indigo-600 dark:hover:text-indigo-400 transition-all border border-transparent hover:border-indigo-500/20"
					>
						Admin
					</Link>

					<ThemeToggle />
				</div>
			</nav>
		</header>
	);
}
