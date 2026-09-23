import { describe, expect, it } from 'vitest';
import { parseSharedSetup } from '@/lib/sharedSetup';

const valid = {
  type: 'nextvestment-transfer-share', version: 1, name: 'Project Share',
  bucket: 'example-transfer-bucket', region: 'ap-southeast-1', prefix: '/project-share/',
  accessKeyId: `AKIA${'A'.repeat(16)}`, secretAccessKey: 'a'.repeat(40),
};

describe('shared setup import', () => {
  it('extracts the key pair and normalizes the project folder', () => {
    expect(parseSharedSetup(JSON.stringify(valid))).toEqual({
      name: 'Project Share', accessKeyId: valid.accessKeyId, secretAccessKey: valid.secretAccessKey,
      share: { bucket: valid.bucket, region: valid.region, prefix: 'project-share/', autoRefresh: true },
    });
  });

  it.each([
    { ...valid, accessKeyId: `ASIA${'A'.repeat(16)}` },
    { ...valid, sessionToken: 'temporary-token' },
    { ...valid, prefix: '' },
    { ...valid, bucket: 's3://example-transfer-bucket' },
    { ...valid, secretAccessKey: 'short' },
  ])('rejects unsupported or incomplete credentials and folders', candidate => {
    expect(() => parseSharedSetup(JSON.stringify(candidate))).toThrow();
  });
});
