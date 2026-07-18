import { Link } from '@tanstack/react-router';
import type { TagId } from '../data/types';
import { cn } from '../lib/cn';

/** A `#tag` chip that links to the feed filtered by that community. */
export function TagBadge({ tag, className }: { tag: TagId; className?: string }) {
  return (
    <Link
      to="/"
      search={{ tag }}
      className={cn(
        'rounded-sm border border-border-1 bg-bg-3 px-1.5 py-px text-[10.5px] text-fg-2 transition-colors hover:border-accent hover:text-accent',
        className
      )}
    >
      #{tag}
    </Link>
  );
}
