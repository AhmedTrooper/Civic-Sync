import { createRouter as createTanStackRouter } from "@tanstack/react-router";
import { routeTree } from "./routeTree.gen";

function NotFound() {
	return (
		<div className="flex min-h-[60vh] flex-col items-center justify-center gap-4 text-center">
			<h2 className="text-4xl font-extrabold tracking-tight text-slate-800 dark:text-slate-100">
				404
			</h2>
			<p className="text-sm text-slate-500 dark:text-slate-400">
				The page you are looking for does not exist.
			</p>
			<a
				href="/"
				className="mt-2 rounded-xl bg-indigo-600 px-5 py-2 text-sm font-semibold text-white shadow hover:bg-indigo-700 transition-colors"
			>
				Back to Dashboard
			</a>
		</div>
	);
}

export function getRouter() {
	const router = createTanStackRouter({
		routeTree,
		scrollRestoration: true,
		defaultPreload: "intent",
		defaultPreloadStaleTime: 0,
		defaultNotFoundComponent: NotFound,
	});

	return router;
}

declare module "@tanstack/react-router" {
	interface Register {
		router: ReturnType<typeof getRouter>;
	}
}
