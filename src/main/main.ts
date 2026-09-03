import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { MarkState, Settings } from '../shared/types';
import { report } from '../shared/report';

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function fillForm(s: Settings) {
  $<HTMLInputElement>('color').value = s.borderColor;
  $<HTMLInputElement>('width').value = String(s.borderWidth);
  $<HTMLInputElement>('radius').value = String(s.borderRadius);
  syncOutputs();
}

function readForm(): Settings {
  return {
    borderColor: $<HTMLInputElement>('color').value,
    borderWidth: Number($<HTMLInputElement>('width').value),
    borderRadius: Number($<HTMLInputElement>('radius').value),
  };
}

function syncOutputs() {
  $('widthv').textContent = $<HTMLInputElement>('width').value;
  $('radiusv').textContent = $<HTMLInputElement>('radius').value;
}

report('get_settings', invoke<Settings>('get_settings').then(fillForm));

/** 颜色选择等低频改动：立即保存；失败时以持久化真值回滚表单 */
async function saveNow() {
  try {
    fillForm(await invoke<Settings>('save_settings', { settings: readForm() }));
  } catch (err) {
    console.error('[markbox:save_settings]', err);
    try {
      fillForm(await invoke<Settings>('get_settings'));
    } catch (revertErr) {
      console.error('[markbox:get_settings]', revertErr);
    }
  }
}

/** 滑块拖动的高频 input：尾部防抖 + 序号守卫，避免每 tick 写盘和乱序回填旧值 */
let saveTimer: number | undefined;
let saveSeq = 0;
function saveDebounced() {
  syncOutputs();
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    const mine = ++saveSeq;
    report('save_settings', invoke<Settings>('save_settings', { settings: readForm() })
      .then((saved) => { if (mine === saveSeq) fillForm(saved); }));
  }, 150);
}

$<HTMLInputElement>('color').addEventListener('input', () => { void saveNow(); });
for (const id of ['width', 'radius']) {
  $<HTMLInputElement>(id).addEventListener('input', saveDebounced);
}

$('start').addEventListener('click', () => report('start_selection', invoke('start_selection')));
$('clear').addEventListener('click', () => report('clear_mark', invoke('clear_mark')));

report('mark-state:listen', listen<MarkState>('mark-state', (e) => {
  $<HTMLButtonElement>('clear').disabled = !e.payload.hasMark;
}));
