import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { useObjects } from '@/hooks/useObjects';
import { objectApi, type ListObjectsResult } from '@/lib/tauri';
import { useProfileStore } from '@/store/profileStore';
import { useProjectShareStore } from '@/store/projectShareStore';
import { useAppStore } from '@/store/appStore';

vi.mock('@/lib/tauri', async importOriginal => {
  const actual = await importOriginal<typeof import('@/lib/tauri')>();
  return { ...actual, objectApi: { ...actual.objectApi, listObjects: vi.fn() } };
});
const listing = (key: string): ListObjectsResult => ({ objects: [{ key, size: 1, last_modified: null, storage_class: null }], common_prefixes: [], next_continuation_token: null, is_truncated: false, prefix: 'team/' });

beforeEach(() => {
  vi.useFakeTimers();
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' });
  Object.defineProperty(navigator, 'onLine', { configurable: true, value: true });
  useProfileStore.getState().setActiveProfileId('a');
  useAppStore.getState().clearDiscoveredRegions();
  useProjectShareStore.setState({ shares: { a: { bucket: 'project-files', region: 'us-east-1', prefix: 'team/', autoRefresh: true } } });
  vi.mocked(objectApi.listObjects).mockReset().mockResolvedValue(listing('initial'));
});
afterEach(() => { vi.useRealTimers(); useProjectShareStore.setState({ shares: {} }); });
const settle = () => act(async () => { await Promise.resolve(); });
const tick = () => act(async () => { await vi.advanceTimersByTimeAsync(15000); });

test('refreshes only the active share without blanking the loaded files, then stops on unmount', async () => {
  const view = renderHook(() => useObjects('project-files', 'us-east-1', 'team/'));
  await settle();
  const pending = Promise.withResolvers<ListObjectsResult>();
  vi.mocked(objectApi.listObjects).mockReturnValueOnce(pending.promise);
  await tick();
  expect(view.result.current.data?.objects[0].key).toBe('initial');
  expect(view.result.current.isLoading).toBe(false);
  await tick();
  expect(objectApi.listObjects).toHaveBeenCalledTimes(2);
  await act(async () => pending.resolve(listing('from-vdi')));
  expect(view.result.current.data?.objects[0].key).toBe('from-vdi');
  expect(vi.mocked(objectApi.listObjects).mock.calls[1][5]).toBe(true);
  view.unmount();
  await tick();
  expect(objectApi.listObjects).toHaveBeenCalledTimes(2);
});

test('pauses hidden or offline views and suspends retries on an authorization error', async () => {
  const view = renderHook(() => useObjects('project-files', 'us-east-1', 'team/'));
  await settle();
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'hidden' });
  await tick();
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' });
  Object.defineProperty(navigator, 'onLine', { configurable: true, value: false });
  await tick();
  expect(objectApi.listObjects).toHaveBeenCalledTimes(1);
  Object.defineProperty(navigator, 'onLine', { configurable: true, value: true });
  vi.mocked(objectApi.listObjects).mockRejectedValueOnce(new Error('Access denied'));
  await tick();
  expect(view.result.current.error).toBe('Access denied');
  await tick(); await tick();
  expect(objectApi.listObjects).toHaveBeenCalledTimes(2);
  vi.mocked(objectApi.listObjects).mockResolvedValue(listing('reconnected'));
  await act(async () => view.result.current.refresh());
  await tick();
  expect(objectApi.listObjects).toHaveBeenCalledTimes(4);
});

test('does not poll outside the configured prefix or after changing profile', async () => {
  const view = renderHook(({ prefix }) => useObjects('project-files', 'us-east-1', prefix), { initialProps: { prefix: 'team-other/' } });
  await settle(); await tick();
  expect(objectApi.listObjects).toHaveBeenCalledTimes(1);
  view.rerender({ prefix: 'team/subfolder/' });
  await settle();
  const pending = Promise.withResolvers<ListObjectsResult>();
  vi.mocked(objectApi.listObjects).mockReturnValueOnce(pending.promise);
  await tick();
  act(() => useProfileStore.getState().setActiveProfileId('b'));
  await settle();
  await act(async () => pending.resolve(listing('wrong-profile')));
  expect(view.result.current.data?.objects[0].key).not.toBe('wrong-profile');
  const calls = vi.mocked(objectApi.listObjects).mock.calls.length;
  await tick();
  expect(objectApi.listObjects).toHaveBeenCalledTimes(calls);
});
