import { useNavigate } from '@tanstack/react-router';

const fieldClass =
  'w-full rounded-md border border-border-2 bg-bg-1 px-2.5 py-2 text-[13px] text-fg-1 outline-none transition focus:border-accent focus:ring-[3px] focus:ring-accent/15';

/** Inline "create a post" form, opened via the `?compose=true` search param. */
export function Composer() {
  const navigate = useNavigate();
  const close = () => navigate({ to: '/', search: prev => ({ ...prev, compose: undefined }) });

  return (
    <form
      onSubmit={e => {
        e.preventDefault();
        close();
      }}
      className="mb-3.5 rounded-lg border border-border-1 bg-bg-2 p-3"
    >
      <div className="mb-2 font-sans text-[13px] font-semibold text-fg-1">create a post</div>
      <input placeholder="title" aria-label="Post title" className={`${fieldClass} mb-2`} />
      <textarea
        placeholder="text or https://url…"
        aria-label="Post body"
        rows={3}
        className={`${fieldClass} resize-none`}
      />
      <div className="mt-2 flex justify-end gap-2">
        <button
          type="button"
          onClick={close}
          className="h-[30px] rounded-md border border-border-2 px-3 text-[12px] text-fg-2 transition-colors hover:bg-bg-3"
        >
          cancel
        </button>
        <button
          type="submit"
          className="h-[30px] rounded-md bg-accent px-3.5 text-[12px] text-white transition-colors hover:bg-accent-hover"
        >
          post
        </button>
      </div>
    </form>
  );
}
