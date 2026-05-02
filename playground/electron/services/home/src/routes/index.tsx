import { Text } from '@cloudflare/kumo';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/')({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <main className="flex flex-col items-center justify-center w-screen h-screen">
      <Text variant="heading1" as="h1">
        Welcome
      </Text>
    </main>
  );
}
