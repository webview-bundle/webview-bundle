import { Loader, Text } from '@cloudflare/kumo';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/')({
  component: IndexComponent,
});

function IndexComponent() {
  return (
    <div className="flex items-center justify-center py-20 gap=4 w-full h-full">
      <Loader size={48} />
      <Text variant="heading3" as="h1">
        Loading...
      </Text>
    </div>
  );
}
