import { createRootRoute, Outlet } from '@tanstack/react-router';
import { Header } from '../components/Header';
import { LeftSidebar } from '../components/LeftSidebar';
import { MobileNav } from '../components/MobileNav';
import { StatusBar } from '../components/StatusBar';
import { cn } from '../lib/cn';
import { AppStateProvider, useAppState } from '../lib/store';

export const Route = createRootRoute({
  component: RootComponent,
});

function RootComponent() {
  return (
    <AppStateProvider>
      <Shell />
    </AppStateProvider>
  );
}

function Shell() {
  const { theme } = useAppState();
  return (
    <div
      data-testid="app-shell"
      data-theme={theme}
      className={cn(
        'flex h-[100dvh] w-full flex-col overflow-hidden bg-bg-1 font-mono text-[14px] text-fg-1',
        // Honor the side safe-area insets (landscape notch / rounded corners);
        // top is handled by the Header, bottom by the MobileNav.
        'pl-[env(safe-area-inset-left)] pr-[env(safe-area-inset-right)]',
        theme === 'dark' && 'dark'
      )}
    >
      <Header />
      <div className="flex min-h-0 flex-1">
        <LeftSidebar />
        <Outlet />
      </div>
      <StatusBar />
      <MobileNav />
    </div>
  );
}
