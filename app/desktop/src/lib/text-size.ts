import { useState } from "react";

/** 文字大小設定（規範 1.4）：改 <html> 的字級，shadcn 和 Tailwind 都用 rem，整個畫面一起放大 */
export type TextSize = "standard" | "large" | "xlarge";

export const TEXT_SIZES: { value: TextSize; label: string; px: number }[] = [
  { value: "standard", label: "標準", px: 16 },
  { value: "large", label: "大", px: 17 },
  { value: "xlarge", label: "特大", px: 18 },
];

const KEY = "text-size";

function saved(): TextSize {
  try {
    const v = localStorage.getItem(KEY);
    return TEXT_SIZES.some((s) => s.value === v) ? (v as TextSize) : "standard";
  } catch {
    return "standard";
  }
}

function apply(size: TextSize) {
  document.documentElement.style.fontSize = `${TEXT_SIZES.find((s) => s.value === size)!.px}px`;
}

/** 開程式時先套用，避免畫面先用標準大小畫一次再跳 */
export function applySavedTextSize() {
  apply(saved());
}

export function useTextSize() {
  const [size, setSize] = useState<TextSize>(saved);
  const change = (next: TextSize) => {
    setSize(next);
    apply(next);
    try {
      localStorage.setItem(KEY, next);
    } catch {
      // 存不起來就只在這次有效
    }
  };
  return [size, change] as const;
}
