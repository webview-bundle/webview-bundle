import { Link } from '@tanstack/react-router';
import { TAGS } from '../data';

const guidelines = [
  '01 · Be terse and technical',
  '02 · Link the spec, not the hype',
  '03 · WIP-honest > marketing',
  '04 · No reposts of the magic number',
];

export function RightRail() {
  return (
    <aside className="hidden w-[288px] flex-shrink-0 flex-col gap-3.5 overflow-y-auto border-l border-border-1 bg-bg-1 p-4 xl:flex">
      <section className="rounded-lg border border-border-1 bg-bg-2 p-3.5">
        <div className="mb-2 flex items-center gap-2">
          <span className="font-sans text-[13px] font-bold tracking-[0.03em] text-fg-1">
            {'WEBVIEW BUNDLE // NEWS'}
          </span>
        </div>
        <p className="m-0 text-[12px] leading-[1.6] text-fg-2">
          The community for <b className="text-fg-1">webview-bundle</b> — offline-first web delivery
          for Electron, Tauri &amp; native webviews. Ship the whole web layer as one signed .wvb.
        </p>
        <div className="my-3 flex gap-[18px] text-[12px]">
          <div>
            <div className="font-sans text-[16px] font-bold text-fg-1">14.2k</div>
            <div className="text-fg-4">members</div>
          </div>
          <div>
            <div className="font-sans text-[16px] font-bold text-success">312</div>
            <div className="text-fg-4">online</div>
          </div>
        </div>
        <Link
          to="/"
          search={prev => ({ ...prev, compose: true })}
          className="flex h-8 w-full items-center justify-center rounded-md bg-accent text-[13px] text-white transition-colors hover:bg-accent-hover"
        >
          + create post
        </Link>
      </section>

      <section className="rounded-lg border border-border-1 p-3.5">
        <div className="mb-2 text-[10.5px] tracking-[0.08em] text-fg-4">GUIDELINES</div>
        <div className="text-[12px] leading-[1.9] text-fg-2">
          {guidelines.map(g => (
            <div key={g}>{g}</div>
          ))}
        </div>
      </section>

      <section className="rounded-lg border border-border-1 p-3.5">
        <div className="mb-2.5 text-[10.5px] tracking-[0.08em] text-fg-4">TRENDING TAGS</div>
        <div className="flex flex-wrap gap-1.5">
          {TAGS.map(tag => (
            <Link
              key={tag}
              to="/"
              search={{ tag }}
              className="rounded-full border border-border-1 bg-bg-2 px-2.5 py-[3px] text-[11px] text-fg-2 transition-colors hover:border-accent hover:text-accent"
            >
              #{tag}
            </Link>
          ))}
        </div>
      </section>
    </aside>
  );
}
