type Listener = () => void;
const openListeners = new Set<Listener>();

/** Imperative trigger so header/sidebar buttons can open the command palette. */
export function openCommandPalette(): void {
  openListeners.forEach((fn) => fn());
}

export function subscribeCommandPalette(listener: Listener): () => void {
  openListeners.add(listener);
  return () => {
    openListeners.delete(listener);
  };
}
