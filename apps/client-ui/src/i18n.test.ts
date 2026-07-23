import { describe, expect, it } from 'vitest';
import { copyFor, otherLocale, translations } from './i18n';

describe('client localization', () => {
  it('keeps Chinese and English keys in lockstep', () => {
    expect(Object.keys(translations.en).sort()).toEqual(
      Object.keys(translations['zh-CN']).sort(),
    );
  });

  it('switches between exactly two selected locales', () => {
    expect(otherLocale('zh-CN')).toBe('en');
    expect(otherLocale('en')).toBe('zh-CN');
    expect(copyFor('zh-CN').now).toBe('现在');
    expect(copyFor('en').now).toBe('Now');
  });
});
