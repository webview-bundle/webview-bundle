import { cn } from '../lib/cn';
import type { VoteDir } from '../lib/store';

export function VoteColumn({
  dir,
  score,
  onUp,
  onDown,
  big = false,
}: {
  dir: VoteDir;
  score: number;
  onUp: () => void;
  onDown: () => void;
  big?: boolean;
}) {
  return (
    <div className="flex flex-col items-center">
      <button
        type="button"
        data-testid="upvote"
        onClick={onUp}
        aria-label="Upvote"
        aria-pressed={dir === 1}
        className={cn(
          'cursor-pointer px-0.5 text-[11px] leading-none transition-colors',
          dir === 1 ? 'text-accent' : 'text-fg-4 hover:text-fg-2'
        )}
      >
        ▲
      </button>
      <span
        data-testid="vote-score"
        className={cn(
          'font-bold leading-normal',
          big ? 'text-[14px]' : 'text-[12px]',
          dir === 1 ? 'text-accent' : dir === -1 ? 'text-downvote' : 'text-fg-2'
        )}
      >
        {score}
      </span>
      <button
        type="button"
        data-testid="downvote"
        onClick={onDown}
        aria-label="Downvote"
        aria-pressed={dir === -1}
        className={cn(
          'cursor-pointer px-0.5 text-[11px] leading-none transition-colors',
          dir === -1 ? 'text-downvote' : 'text-fg-4 hover:text-fg-2'
        )}
      >
        ▼
      </button>
    </div>
  );
}
