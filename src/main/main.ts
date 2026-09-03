import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { MarkState, Settings } from '../shared/types';

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function fillForm(s: Settings) {
  $<HTMLInputElement>('color').value = s.borderColor;
  $<HTMLInputElement>('width').value = String(s.borderWidth);
  $<HTMLInputElement>('radius').value = String(s.borderRadius);
  $('widthv').textContent = String(s.borderWidth);
  $('radiusv').textContent = String(s.borderRadius);
}

function readForm(): Settings {
  return {
    borderColor: $<HTMLInputElement>('color').value,
    borderWidth: Number($<HTMLInputElement>('width').value),
    borderRadius: Number($<HTMLInputElement>('radius').value),
  };
}

invoke<Settings>('get_settings').then(fillForm);

for (const id of ['color', 'width', 'radius']) {
  const el = $<HTMLInputElement>(id);
  el.addEventListener('input', async () => {
    const saved = await invoke<Settings>('save_settings', { settings: readForm() });
    fillForm(saved);
  });
}

$('start').addEventListener('click', () => invoke('start_selection'));
$('clear').addEventListener('click', () => invoke('clear_mark'));

void listen<MarkState>('mark-state', (e) => {
  $<HTMLButtonElement>('clear').disabled = !e.payload.hasMark;
});
