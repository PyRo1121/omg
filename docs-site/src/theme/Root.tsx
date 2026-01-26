/**
 * Root theme component - wraps entire Docusaurus app
 * Initializes analytics and global features
 */

import React, { useEffect } from 'react';
import { initAnalytics } from '../lib/analytics';

export default function Root({ children }: { children: React.ReactNode }) {
  useEffect(() => {
    // Initialize analytics on mount
    const analytics = initAnalytics();

    // Cleanup on unmount
    return () => {
      analytics?.destroy();
    };
  }, []);

  return <>{children}</>;
}
