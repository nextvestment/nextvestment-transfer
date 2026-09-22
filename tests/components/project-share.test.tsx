import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import ProjectShareCard from '@/components/share/ProjectShareCard';
import { projectSharePath, useProjectShareStore, validateProjectShare } from '@/store/projectShareStore';

const push = vi.hoisted(() => vi.fn());
vi.mock('next/navigation', () => ({ useRouter: () => ({ push }) }));

beforeEach(() => {
  localStorage.clear();
  useProjectShareStore.setState({ shares: {} });
  push.mockClear();
});

test('saves a nonsecret profile-scoped folder and opens its encoded path directly', () => {
  render(<ProjectShareCard profileId="profile-a" profileName="Work SSO" defaultRegion="ap-southeast-1" />);
  fireEvent.change(screen.getByLabelText(/Bucket name/), { target: { value: 'project-files' } });
  fireEvent.change(screen.getByLabelText('Folder prefix'), { target: { value: '/A & B/reports/' } });
  fireEvent.click(screen.getByRole('button', { name: 'Save shared folder' }));
  expect(useProjectShareStore.getState().shares['profile-a']).toEqual({ bucket: 'project-files', region: 'ap-southeast-1', prefix: 'A & B/reports/', autoRefresh: true });
  fireEvent.click(screen.getByRole('button', { name: 'Open Project Share' }));
  expect(push).toHaveBeenCalledWith('/bucket?name=project-files&region=ap-southeast-1&prefix=A+%26+B%2Freports%2F');
  const stored = JSON.parse(localStorage.getItem('nextvestment-project-shares-v1')!);
  expect(Object.keys(stored.state)).toEqual(['shares']);
  expect(Object.keys(stored.state.shares['profile-a']).sort()).toEqual(['autoRefresh', 'bucket', 'prefix', 'region']);
});

test('rejects a URI in the bucket field and leaves the setup unsaved', () => {
  render(<ProjectShareCard profileId="a" profileName="Profile" defaultRegion="ap-southeast-1" />);
  fireEvent.change(screen.getByLabelText(/Bucket name/), { target: { value: 's3://project-files/' } });
  fireEvent.click(screen.getByRole('button', { name: 'Save shared folder' }));
  expect(screen.getByRole('alert').textContent).toContain('bucket name only');
  expect(useProjectShareStore.getState().shares).toEqual({});
});

test('switching profiles does not expose another profile share and forget only removes that setup', () => {
  useProjectShareStore.getState().saveShare('a', { bucket: 'project-files', region: 'us-east-1', prefix: 'team/', autoRefresh: false });
  const view = render(<ProjectShareCard key="a" profileId="a" profileName="First" defaultRegion="us-east-1" />);
  fireEvent.click(screen.getByRole('button', { name: 'Forget setup' }));
  expect(useProjectShareStore.getState().shares.a).toBeUndefined();
  useProjectShareStore.getState().saveShare('a', { bucket: 'project-files', region: 'us-east-1', prefix: 'team/', autoRefresh: false });
  view.rerender(<ProjectShareCard key="b" profileId="b" profileName="Second" defaultRegion="us-west-2" />);
  expect(screen.queryByRole('button', { name: 'Open Project Share' })).toBeNull();
  expect((screen.getByLabelText(/Bucket name/) as HTMLInputElement).value).toBe('');
  expect(useProjectShareStore.getState().shares.a.bucket).toBe('project-files');
});

test('folder normalization preserves a safe encoded prefix and drops unknown fields', () => {
  const share = validateProjectShare({ bucket: 'project-files', region: 'us-gov-west-1', prefix: 'release #1/', autoRefresh: false, secret: 'not-persisted' } as Parameters<typeof validateProjectShare>[0]);
  expect(share).not.toHaveProperty('secret');
  expect(projectSharePath(share)).toContain('prefix=release+%231%2F');
  expect(() => validateProjectShare({ ...share, region: 'https://example.com' })).toThrow('AWS region');
});
