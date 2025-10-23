import { api } from './api';
import { useStore } from '../state/store';

// Initialize global watch event listener
// This should be called once when the app starts
export function initWatchEvents() {
  api.onWatchEvent(async (event) => {
    const store = useStore.getState();
    const { activeRepoId, refreshStatus, selectedPath, currentDiffSide, entries, clearDiff } = store;

    // Only process events for the currently active repo
    if (event.repoId === activeRepoId) {
      // Refresh status silently (no loading indicator) to avoid flicker
      await refreshStatus(event.repoId, true);

      // If a file is currently selected, reload its diff to keep it in sync
      if (selectedPath && currentDiffSide) {
        // Check if the selected file still exists in the new status
        const fileStillExists = entries.some(e => e.path === selectedPath);

        if (fileStillExists) {
          // Silently reload the diff for the currently selected file
          const response = await api.getDiff({
            repoId: event.repoId,
            path: selectedPath,
            side: currentDiffSide,
          });

          if (response.ok && response.data) {
            useStore.setState({ currentDiff: response.data });
          }
        } else {
          // File was deleted or no longer has changes, clear the diff
          clearDiff();
        }
      }
    }
  });
}
