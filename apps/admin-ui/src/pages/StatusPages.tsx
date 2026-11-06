export function Forbidden() {
  return <main className="p-10"><h1 className="text-3xl font-semibold">403</h1><p>Der Admin-Zugang erfordert eine Staff-Sitzung und die passende Modellberechtigung.</p></main>;
}

export function NotFound() {
  return <main className="p-10"><h1 className="text-3xl font-semibold">404</h1><p>Diese Admin-Seite existiert nicht.</p></main>;
}
