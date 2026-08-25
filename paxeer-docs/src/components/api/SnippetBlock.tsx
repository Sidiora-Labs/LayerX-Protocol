'use client'

import { useState } from 'react'
import { m3 } from './tokens'

export type SnippetMeta = {
  method: string
  args: string
  source: string
  purpose: string
  code: string
}

function copyText(text: string) {
  return navigator.clipboard.writeText(text).catch(() => {
    const field = document.createElement('textarea')
    field.value = text
    field.style.position = 'fixed'
    field.style.opacity = '0'
    document.body.appendChild(field)
    field.select()
    document.execCommand('copy')
    field.remove()
  })
}

function ActionButton({
  label,
  doneLabel,
  onCopy,
}: {
  label: string
  doneLabel: string
  onCopy: () => Promise<void> | void
}) {
  const [copied, setCopied] = useState(false)

  return (
    <button
      type="button"
      onClick={async () => {
        await onCopy()
        setCopied(true)
        setTimeout(() => setCopied(false), 2000)
      }}
      className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-surface-high text-on-surface-variant hover:text-on-surface text-xs font-medium transition-all duration-150"
    >
      {copied ? doneLabel : label}
    </button>
  )
}

export function SnippetBlock({ method, args, source, purpose, code }: SnippetMeta) {
  const agentText = [
    `method: ${method}`,
    `args: ${args}`,
    `source: ${source}`,
    `purpose: ${purpose}`,
  ].join('\n')

  return (
    <div className="my-6 rounded-lg bg-surface-container shadow-1 overflow-hidden">
      <div className="flex flex-wrap items-start justify-between gap-3 px-4 py-3 bg-surface-low">
        <div className="min-w-0">
          <div className={m3.overline}>{method}</div>
          <p className={`${m3.label} mt-1`}>{purpose}</p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <ActionButton label="Copy" doneLabel="Copied" onCopy={() => copyText(code)} />
          <ActionButton label="Agent copy" doneLabel="Copied" onCopy={() => copyText(agentText)} />
        </div>
      </div>
      <div className="px-4 py-3 font-mono text-sm text-on-surface overflow-x-auto whitespace-pre">{code}</div>
      <div className="px-4 py-2 bg-surface-low">
        <span className={m3.label}>
          {source}
          {args ? ` · ${args}` : ''}
        </span>
      </div>
    </div>
  )
}
