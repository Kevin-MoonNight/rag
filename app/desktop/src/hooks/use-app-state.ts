import { useCallback, useEffect, useState } from "react";
import { getSources, getStatus, SourceEntry, Status } from "@/api";
import { errorText } from "@/lib/format";

/** 索引狀態：建立索引時每秒更新一次進度，平常每十秒更新一次 */
export function useStatus() {
  const [status, setStatus] = useState<Status | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    getStatus()
      .then((s) => {
        setStatus(s);
        setError(null);
      })
      .catch((e) => setError(errorText(e)));
  }, []);

  const running = !!status?.progress.running;
  useEffect(() => {
    refresh();
    const timer = window.setInterval(refresh, running ? 1000 : 10000);
    return () => window.clearInterval(timer);
  }, [refresh, running]);

  return { status, error, refresh };
}

/** 使用者加入的資料夾和檔案；建索引開始或結束時重新讀（離線狀態可能變了） */
export function useSources(running: boolean) {
  const [sources, setSources] = useState<SourceEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    getSources()
      .then((list) => {
        setSources(list);
        setError(null);
      })
      .catch((e) => setError(errorText(e)));
  }, []);

  useEffect(reload, [reload, running]);

  return { sources, error, reload };
}
