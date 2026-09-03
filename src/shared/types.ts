// 命名约定：width/height = 屏幕级物理矩形，对齐 Rust MonitorRect；w/h = 选区矩形。勿强行统一。
export interface Settings {
  borderColor: string;
  borderWidth: number; // 1-10
  borderRadius: number; // 0-16
}
export interface MonitorRect { x: number; y: number; width: number; height: number } // 物理像素，全局坐标
export interface OverlayInit { monitor: MonitorRect }
export interface PhysRect { x: number; y: number; w: number; h: number } // 物理像素，全局坐标
export interface ConfirmPayload { rect: PhysRect }
export interface MarkState { hasMark: boolean }
