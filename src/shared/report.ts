/** invoke/listen 兜底上报：所有无人 await 的 promise 都须经此或自行 catch，避免静默的 unhandled rejection */
export function report(label: string, p: Promise<unknown>): void {
  p.catch((err) => console.error(`[markbox:${label}]`, err));
}
