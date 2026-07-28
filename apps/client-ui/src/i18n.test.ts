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

  it('localizes first-use, runtime, feedback, and error paths', () => {
    expect(copyFor('zh-CN').joinCommunity).toBe('加入朋友的社区');
    expect(copyFor('en').joinCommunity).toBe('Join a friend’s community');
    expect(copyFor('zh-CN').connection('degraded')).toBe('本地模式');
    expect(copyFor('en').connection('degraded')).toBe('Local mode');
    expect(copyFor('zh-CN').errorMessage('nodeUnavailable')).toContain('协议组件');
    expect(copyFor('en').errorMessage('nodeUnavailable')).toContain('protocol component');
    expect(copyFor('zh-CN').errorMessage('nodeTimeout')).toContain('响应超时');
    expect(copyFor('en').errorMessage('nodeTimeout')).toContain('took too long');
    expect(copyFor('zh-CN').feedbackTitle).toBe('反馈与问题');
    expect(copyFor('en').feedbackTitle).toBe('Feedback & issues');
    expect(copyFor('zh-CN').voiceChecking).toContain('检查麦克风');
    expect(copyFor('en').voiceChecking).toContain('Checking microphone');
    expect(copyFor('zh-CN').errorMessage('voicePermissionMac')).toContain('隐私与安全性');
    expect(copyFor('en').errorMessage('voicePermissionWindows')).toContain('Privacy & security');
    expect(copyFor('zh-CN').errorMessage('voiceMicrophoneMissing')).toContain('没有找到麦克风');
    expect(copyFor('en').errorMessage('voiceMicrophoneBusy')).toContain('another app');
  });
});
