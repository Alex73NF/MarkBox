import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Settings } from '../shared/types';
import { report } from '../shared/report';

const box = document.getElementById('box')!;

function apply(s: Settings) {
  box.style.borderWidth = `${s.borderWidth}px`;
  box.style.borderColor = s.borderColor;
  box.style.borderRadius = `${s.borderRadius}px`;
  box.style.visibility = 'visible'; // 设置就绪后再显示，避免 HTML 内默认样式闪现
}

report('get_settings', invoke<Settings>('get_settings').then(apply));
report('settings-updated:listen', listen<Settings>('settings-updated', (e) => apply(e.payload)));
