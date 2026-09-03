import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Settings } from '../shared/types';

const box = document.getElementById('box')!;

function apply(s: Settings) {
  box.style.borderWidth = `${s.borderWidth}px`;
  box.style.borderColor = s.borderColor;
  box.style.borderRadius = `${s.borderRadius}px`;
}

invoke<Settings>('get_settings').then(apply);
void listen<Settings>('settings-updated', (e) => apply(e.payload));
