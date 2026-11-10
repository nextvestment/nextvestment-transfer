import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import ReceiveFileCard from '@/components/share/ReceiveFileCard';
import { receiveHandoff } from '@/lib/tauri';

const save = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/plugin-dialog', () => ({ save }));
vi.mock('@/lib/tauri', () => ({ receiveHandoff: vi.fn() }));

const LINK = 'https://files.s3.amazonaws.com/source.zip?X-Amz-Signature=synthetic';

beforeEach(() => {
  vi.clearAllMocks();
  save.mockResolvedValue('/tmp/received.zip');
});

test('receives to the chosen path, shows the receipt and forgets the bearer link', async () => {
  vi.mocked(receiveHandoff).mockResolvedValue({ bytes: 1234, sha256: 'ab'.repeat(32) });
  render(<ReceiveFileCard />);
  const button = screen.getByRole('button', { name: 'Choose destination and receive' });
  expect((button as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(screen.getByLabelText(/Private download link/), { target: { value: ` ${LINK} ` } });
  fireEvent.click(button);
  expect(await screen.findByText(/File received: 1,234 bytes/)).toBeTruthy();
  expect(receiveHandoff).toHaveBeenCalledWith(LINK, '/tmp/received.zip', undefined);
  expect((screen.getByLabelText(/Private download link/) as HTMLInputElement).value).toBe('');
});

test('a failed receive never echoes the native error or the link', async () => {
  vi.mocked(receiveHandoff).mockRejectedValue(new Error(`GET ${LINK} failed at /Users/me/secret`));
  render(<ReceiveFileCard />);
  fireEvent.change(screen.getByLabelText(/Private download link/), { target: { value: LINK } });
  fireEvent.click(screen.getByRole('button', { name: 'Choose destination and receive' }));
  const alert = await screen.findByRole('alert');
  expect(alert.textContent).toContain('Could not receive the file.');
  expect(document.body.textContent).not.toContain('X-Amz-Signature');
  expect(document.body.textContent).not.toContain('/Users/me');
});

test('cancelling the save dialog does not start a download', async () => {
  save.mockResolvedValue(null);
  render(<ReceiveFileCard />);
  fireEvent.change(screen.getByLabelText(/Private download link/), { target: { value: LINK } });
  fireEvent.click(screen.getByRole('button', { name: 'Choose destination and receive' }));
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(receiveHandoff).not.toHaveBeenCalled();
});
