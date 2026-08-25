import Link from 'next/link'
import { ReactNode } from 'react'
import { SnippetBlock } from './SnippetBlock'
import { m3 } from './tokens'

export { SnippetBlock, m3 }

export function PageLead({
  overline,
  children,
  source,
}: {
  overline: string
  children: ReactNode
  source: string
}) {
  return (
    <header className="mb-10">
      <p className={`${m3.overline} mb-4`}>{overline}</p>
      <div className={`${m3.body} space-y-4`}>{children}</div>
      <div className="mt-6 rounded-lg bg-surface-low px-4 py-3">
        <div className={m3.label}>Source</div>
        <code className="text-sm">{source}</code>
      </div>
    </header>
  )
}

export function FactChips({ items }: { items: { label: string; value: string }[] }) {
  return (
    <div className="grid grid-cols-2 gap-3 mb-10">
      {items.map((item) => (
        <div key={item.label} className="rounded-lg bg-surface-low px-4 py-4 shadow-1">
          <div className={`${m3.label} mb-2`}>{item.label}</div>
          <div className="font-mono text-sm font-medium text-on-surface">{item.value}</div>
        </div>
      ))}
    </div>
  )
}

export function JumpNav({ items }: { items: { id: string; label: string }[] }) {
  return (
    <nav className="mb-10" aria-label="On this page">
      <div className={`${m3.label} mb-3`}>On this page</div>
      <div className="flex flex-wrap gap-2">
        {items.map((item) => (
          <a
            key={item.id}
            href={`#${item.id}`}
            className="inline-flex items-center min-h-9 px-3 rounded-full bg-surface-low text-sm text-on-surface-variant hover:text-on-surface hover:bg-surface-high transition-all duration-150"
          >
            {item.label}
          </a>
        ))}
      </div>
    </nav>
  )
}

export function Section({
  id,
  title,
  children,
}: {
  id: string
  title: string
  children: ReactNode
}) {
  return (
    <section className="mb-14">
      <h2
        id={id}
        className={`${m3.headline} scroll-mt-8 !mt-0 !mb-3 !border-0 !pb-0`}
        style={{ borderBottom: 'none', paddingBottom: 0, marginTop: 0 }}
      >
        {title}
      </h2>
      <div className="h-px bg-outline-variant mb-6" />
      {children}
    </section>
  )
}

export function Subhead({
  id,
  children,
}: {
  id?: string
  children: ReactNode
}) {
  return (
    <h3
      id={id}
      className={`${m3.title} scroll-mt-8 !mt-8 !mb-3`}
      style={{ marginTop: '2rem' }}
    >
      {children}
    </h3>
  )
}

export function MethodTable({
  columns,
  rows,
}: {
  columns: string[]
  rows: string[][]
}) {
  return (
    <div className="my-6 rounded-lg bg-surface-container shadow-1 overflow-hidden">
      <div
        className="grid gap-3 px-4 py-3 bg-surface-low"
        style={{ gridTemplateColumns: `repeat(${columns.length}, minmax(0, 1fr))` }}
      >
        {columns.map((column) => (
          <div key={column} className={`${m3.label} text-on-surface`}>
            {column}
          </div>
        ))}
      </div>
      {rows.map((row, index) => (
        <div
          key={`${row[0]}-${index}`}
          className={`grid gap-3 px-4 py-3 ${index % 2 === 0 ? 'bg-surface-container' : 'bg-surface-low'}`}
          style={{ gridTemplateColumns: `repeat(${columns.length}, minmax(0, 1fr))` }}
        >
          {row.map((cell, cellIndex) => (
            <div
              key={`${index}-${cellIndex}`}
              className={cellIndex === 0 ? 'font-mono text-sm text-on-surface break-all' : `${m3.body} text-sm`}
            >
              {cell}
            </div>
          ))}
        </div>
      ))}
    </div>
  )
}

export function Callout({
  label,
  children,
}: {
  label: string
  children: ReactNode
}) {
  return (
    <div className="my-6 rounded-lg bg-surface-low px-4 py-3 shadow-1">
      <div className={`${m3.label} mb-1`}>{label}</div>
      <div className={m3.body}>{children}</div>
    </div>
  )
}

export function PageNav({
  prev,
  next,
}: {
  prev?: { href: string; title: string }
  next?: { href: string; title: string }
}) {
  return (
    <div className="mt-16 grid grid-cols-2 gap-3">
      {prev ? (
        <Link href={prev.href} className="rounded-lg bg-surface-container shadow-1 px-5 py-4 hover:bg-surface-high transition-all duration-150">
          <div className={m3.label}>Previous</div>
          <div className={`${m3.title} mt-1`}>{prev.title}</div>
        </Link>
      ) : (
        <div />
      )}
      {next ? (
        <Link href={next.href} className="rounded-lg bg-surface-container shadow-1 px-5 py-4 text-right hover:bg-surface-high transition-all duration-150">
          <div className={m3.label}>Next</div>
          <div className={`${m3.title} mt-1`}>{next.title}</div>
        </Link>
      ) : (
        <div />
      )}
    </div>
  )
}
