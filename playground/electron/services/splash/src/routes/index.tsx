import { Button, Loader, Text } from '@cloudflare/kumo';
import { createFileRoute } from '@tanstack/react-router';
import { useEffect, useState } from 'react';

export const Route = createFileRoute('/')({
  component: IndexComponent,
});

function IndexComponent() {
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    const timeoutId = window.setTimeout(() => {
      setLoaded(true);
    }, 2_500);

    return () => {
      window.clearTimeout(timeoutId);
    };
  }, []);

  const navigateToHome = () => {
    window.location.href = 'app://home.wvb';
  };

  return (
    <div className="flex flex-col items-center justify-center py-20 gap-4 w-full h-full">
      {loaded ? (
        <>
          <Text variant="heading2" as="h1">
            Welcome to Webview Bundle Playground
          </Text>
          <Button variant="primary" size="lg" onClick={navigateToHome}>
            Go to Home
          </Button>
        </>
      ) : (
        <>
          <Loader size={48} />
          <Text variant="heading3" as="h1">
            Loading...
          </Text>
        </>
      )}
    </div>
  );
}
