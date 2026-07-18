import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { version } from '../package.json' with { type: 'json' };

function App() {
  return <h1 data-testid="version">{version}</h1>;
}

const root = createRoot(document.getElementById('root')!);
root.render(
  <StrictMode>
    <App />
  </StrictMode>
);
