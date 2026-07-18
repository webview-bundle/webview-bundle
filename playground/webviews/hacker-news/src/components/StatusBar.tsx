/** Webview-flavored status bar — evokes a native app mounting the .wvb bundle. */
export function StatusBar() {
  return (
    <div className="hidden h-[27px] flex-shrink-0 items-center gap-3.5 border-t border-border-1 bg-bg-2 px-3.5 text-[11px] text-fg-3 lg:flex">
      <span className="flex items-center gap-1.5">
        <span className="h-[7px] w-[7px] rounded-full bg-success shadow-[0_0_0_2px_rgba(22,163,74,0.18)]" />
        connected
      </span>
      <span className="text-fg-4">remote Source · news.wvb.dev</span>
      <span className="text-fg-4">builtin fallback ready</span>
      <span className="ml-auto text-fg-4">⟳ synced 2m ago</span>
      <span className="text-fg-4">@wvb/web v1.4.0</span>
    </div>
  );
}
