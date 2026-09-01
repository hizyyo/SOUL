import { describe, it, expect } from 'vitest';
import manifestJson from '../browser/manifest.source.json';
import { EXTENSION_ID, SUPPORTED_ORIGINS } from '../browser/src/constants';

interface Manifest {
  manifest_version: number;
  version?: string;
  version_name?: string;
  key?: string;
  permissions?: string[];
  host_permissions?: string[];
  content_scripts?: { matches?: string[]; js?: string[] }[];
  background?: { service_worker?: string };
  icons?: Record<string, string>;
  action?: Record<string, unknown>;
  oauth2?: unknown;
  externally_connectable?: unknown;
}

const manifest = manifestJson as Manifest;

async function extensionIdFromKey(keyB64: string): Promise<string> {
  const der = new Uint8Array(
    atob(keyB64)
      .split('')
      .map((c) => c.charCodeAt(0)),
  );
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', der));
  const nibbles = [...digest.subarray(0, 16)].flatMap((byte) => [byte >> 4, byte & 0x0f]);
  return nibbles.map((nibble) => String.fromCharCode(0x61 + nibble)).join('');
}

describe('manifest.source.json', () => {
  it('manifest_version 3', () => {
    expect(manifest.manifest_version).toBe(3);
  });

  it('uses a store-compatible version and the current pre-release name', () => {
    expect(manifest.version).toBe('0.2.0.1');
    expect(manifest.version_name).toBe('0.2.0-alpha.1');
  });

  it('ID расширения, вычисленный из ключа, совпадает с константой', async () => {
    const key = manifest.key;
    expect(typeof key).toBe('string');
    expect(key && key.length > 100).toBe(true);
    expect(key).toBeDefined();
    if (key) {
      expect(await extensionIdFromKey(key)).toBe(EXTENSION_ID);
    }
  });

  it('не запрашивает storage, tabs, cookies или другие лишние разрешения', () => {
    expect(manifest.permissions).toEqual(['nativeMessaging']);
  });

  it('host_permissions и content_scripts покрывают ровно поддерживаемые сайты', () => {
    const matches = manifest.content_scripts?.[0]?.matches ?? [];
    const expected = SUPPORTED_ORIGINS.map((origin) => `${origin}/*`);
    expect(manifest.host_permissions).toEqual(expected);
    expect(matches).toEqual(expected);
  });

  it('использует классические скрипты (не модули) и иконки всех размеров', () => {
    expect(manifest.background?.service_worker).toBe('background.js');
    expect(manifest.content_scripts?.[0]?.js).toEqual(['content.js']);
    for (const size of [16, 32, 48, 128]) {
      expect(manifest.icons?.[String(size)]).toBe(`icons/${size}.png`);
    }
  });

  it('не содержит popup, веб-доступ к данным или внешние ключи связи', () => {
    expect(manifest.action?.default_popup).toBeUndefined();
    expect(manifest.oauth2).toBeUndefined();
    expect(manifest.externally_connectable).toBeUndefined();
  });
});
