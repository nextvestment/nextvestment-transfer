import { createJSONStorage } from 'zustand/middleware';

export const browserStorage = createJSONStorage(() => {
  if (typeof window === 'undefined') {
    throw new Error('Browser storage is unavailable during server rendering');
  }

  return window.localStorage;
});
