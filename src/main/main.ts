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

let saveTimer: number | undefined;
let saveSeq = 0;
// 单飞串行链：async 命令在线程池上执行，到达序 ≠ 完成序，并发保存可能让旧快照最后落盘；
// 链上各环按发起顺序执行、且执行时才读表单，保证最后落盘的必是最新值
let saveChain: Promise<void> = Promise.resolve();

/** 保存当前表单并回填归一化结果；序号失配（已有更新的保存排队）时不回填；
 *  失败以持久化真值回滚。立即保存与防抖保存共用同一出口，失败语义一致。
 *  序号/防抖时序属手动验证项（模块含 DOM，vitest 不加载），修改时逐案核对 */
async function persist(mine: number) {
  try {
    const saved = await invoke<Settings>('save_settings', { settings: readForm() });
    if (mine === saveSeq) fillForm(saved);
  } catch (err) {
    console.error('[markbox:save_settings]', err);
    try {
      const truth = await invoke<Settings>('get_settings');
      if (mine === saveSeq) fillForm(truth); // 已有更新的保存落定：磁盘真值可能就是用户后续输入，别踩掉表单
    } catch (revertErr) {
      console.error('[markbox:get_settings]', revertErr);
    }
  }
}

function enqueueSave() {
  const mine = ++saveSeq;
  saveChain = saveChain.then(() => persist(mine));
}

/** 颜色选择等低频改动：立即保存 */
function saveNow() {
  enqueueSave(); // 递增序号使在途回填失效，防止旧快照晚到回填进表单
}

/** 滑块拖动的高频 input：尾部防抖，避免每 tick 写盘和乱序回填旧值 */
function saveDebounced() {
  syncOutputs();
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(enqueueSave, 150);
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
