import { render } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import Sidebar from '@/components/layout/Sidebar';
import Footer from '@/components/layout/Footer';
import { useBuckets } from '@/hooks/useBuckets';
import { useProfileStore } from '@/store/profileStore';

const query = vi.hoisted(() => ({ value: '' }));
vi.mock('next/navigation', () => ({ useRouter: () => ({ push: vi.fn() }), usePathname: () => '/bucket', useSearchParams: () => new URLSearchParams(query.value) }));
vi.mock('@/hooks/useBuckets', () => ({ useBuckets: vi.fn(() => ({ buckets: [], isLoading: false, fetchBuckets: vi.fn(), isCached: false, cacheAge: null })) }));

beforeEach(() => {
  query.value = 'name=project-files&region=us-east-1&prefix=team%2F';
  useProfileStore.setState({ activeProfileId: 'a', profiles: [{ id: 'a', name: 'Work', credential_type: { type: 'SharedConfig', profile_name: 'work' }, is_default: true, region: 'us-east-1' }] });
});

test('sidebar and footer do not request account-wide discovery when a folder opens directly', () => {
  render(<><Sidebar /><Footer /></>);
  for (const [options] of vi.mocked(useBuckets).mock.calls) expect(options?.enabled).toBe(false);
});

test('account-wide discovery requires the explicit discovery route', () => {
  query.value = 'view=discovery';
  render(<><Sidebar /><Footer /></>);
  for (const [options] of vi.mocked(useBuckets).mock.calls) expect(options?.enabled).toBe(true);
});
