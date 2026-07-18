import * as Radix from '@radix-ui/react-scroll-area';
import type { ReactNode } from 'react';
import { cn } from '../lib/cn';

interface ScrollAreaProps {
  /**
   * Forwarded to the underlying `<main>` so the E2E suite's `data-testid`
   * hooks (`feed`, `post-detail`, `profile`) keep resolving to the scroll root.
   */
  'data-testid'?: string;
  /** Extra classes on the scroll root (the flex child that fills the column). */
  className?: string;
  /** Extra classes on the scrolling viewport (inner padding, etc.). */
  viewportClassName?: string;
  children: ReactNode;
}

/**
 * A page-level scroll region with auto-hiding overlay scrollbars (Radix
 * `type="scroll"`): the bar fades in while the user scrolls and disappears once
 * they stop, instead of a persistent gutter — closer to a native webview feel.
 *
 * Renders as `<main>` so each route keeps its landmark element and the
 * `data-testid` the tests drive. Native scrollbars are hidden by Radix; the
 * thumb below matches the app's existing terminal-neutral scrollbar styling.
 */
export function ScrollArea({ className, viewportClassName, children, ...rest }: ScrollAreaProps) {
  return (
    <Radix.Root
      asChild
      type="scroll"
      scrollHideDelay={500}
      className={cn('min-w-0 flex-1 overflow-hidden', className)}
    >
      <main {...rest}>
        {/* `[&>div]:!block` overrides Radix's inner `display:table` content
            wrapper. For a vertical-only region that keeps sticky children (the
            mobile feed controls) sticking and `mx-auto` content centered.
            `overscroll-contain` stops a fling from chaining to the locked shell
            (Radix already adds `-webkit-overflow-scrolling: touch`). */}
        <Radix.Viewport
          className={cn('size-full overscroll-contain [&>div]:!block', viewportClassName)}
        >
          {children}
        </Radix.Viewport>
        <Radix.Scrollbar
          orientation="vertical"
          className="z-[3] flex w-2.5 touch-none select-none p-[2px]"
        >
          <Radix.Thumb className="flex-1 rounded-full bg-border-2 transition-colors hover:bg-fg-4" />
        </Radix.Scrollbar>
      </main>
    </Radix.Root>
  );
}
