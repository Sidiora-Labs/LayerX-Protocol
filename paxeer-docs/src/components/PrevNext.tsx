import Link from 'next/link'

export function PrevNext({
  prev,
  next,
}: {
  prev: { href: string; title: string }
  next: { href: string; title: string }
}) {
  return (
    <nav className="grid grid-cols-2 gap-3 mt-12 pt-8 border-t border-outline-variant">
      <Link
        href={prev.href}
        className="bg-surface-high rounded-lg p-5 border border-outline-variant hover:border-ink-text transition-all duration-150 hover:translate-y-[-2px]"
      >
        <div className="text-xs text-on-surface-variant uppercase tracking-wider mb-2">Previous</div>
        <div className="text-lg font-medium text-on-surface">{prev.title}</div>
      </Link>
      <Link
        href={next.href}
        className="bg-surface-high rounded-lg p-5 border border-outline-variant hover:border-ink-text transition-all duration-150 hover:translate-y-[-2px] text-right"
      >
        <div className="text-xs text-on-surface-variant uppercase tracking-wider mb-2">Next</div>
        <div className="text-lg font-medium text-on-surface">{next.title}</div>
      </Link>
    </nav>
  )
}
