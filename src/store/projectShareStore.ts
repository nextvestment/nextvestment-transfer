'use client';

import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { browserStorage } from './browserStorage';

export interface ProjectShare {
  bucket: string;
  region: string;
  prefix: string;
  autoRefresh: boolean;
}

export function validateProjectShare(input: ProjectShare): ProjectShare {
  const bucket = input.bucket.trim();
  const region = input.region.trim();
  const prefix = input.prefix.trim().replace(/^\/+|\/+$/g, '');
  if (!/^[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]$/.test(bucket) || bucket.includes('..')) {
    throw new Error('Enter the bucket name only, without s3:// or a folder path.');
  }
  if (!/^[a-z]{2}(?:-[a-z]+)+-\d+$/.test(region)) {
    throw new Error('Enter the AWS region supplied for this share, for example ap-southeast-1.');
  }
  if (/[\x00-\x1f\x7f\\]/.test(prefix)) {
    throw new Error('Use forward slashes in the folder prefix, without control characters.');
  }
  return { bucket, region, prefix: prefix ? `${prefix}/` : '', autoRefresh: input.autoRefresh === true };
}

export function projectSharePath(share: ProjectShare): string {
  const params = new URLSearchParams({ name: share.bucket, region: share.region });
  if (share.prefix) params.set('prefix', share.prefix);
  return `/bucket?${params.toString()}`;
}

export function isProjectShareLocation(share: ProjectShare | undefined, bucket: string, region: string | undefined, prefix: string): boolean {
  return !!share && share.bucket === bucket && share.region === region && prefix.startsWith(share.prefix);
}

interface ProjectShareState {
  shares: Record<string, ProjectShare>;
  saveShare: (profileId: string, input: ProjectShare) => void;
  removeShare: (profileId: string) => void;
}

export const useProjectShareStore = create<ProjectShareState>()(persist((set) => ({
  shares: {},
  saveShare: (profileId, input) => {
    if (!profileId) throw new Error('Select an AWS profile first.');
    const share = validateProjectShare(input);
    set(state => ({ shares: { ...state.shares, [profileId]: share } }));
  },
  removeShare: profileId => set(state => {
    const shares = { ...state.shares };
    delete shares[profileId];
    return { shares };
  }),
}), {
  name: 'nextvestment-project-shares-v1',
  storage: browserStorage,
  partialize: state => ({ shares: state.shares }),
}));
