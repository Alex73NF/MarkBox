/** Vite 的 ?raw 原文导入（vitest 同管线）；无 vite/client 全局类型，本地补最小声明 */
declare module '*?raw' {
  const content: string;
  export default content;
}
