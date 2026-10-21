# 馬上停掉 Windows 上所有的量測（win_* 腳本、rag.exe、rag-desktop.exe、llama-server.exe、量測用的 Docker 容器）。
# 做完的步驟有 .done 標記，之後重跑 win_80_idle_suite.ps1 會從停下的那一步接著做。
# 用法：powershell -File C:\Users\k0937\Desktop\rag\experiments\win_stop_suite.ps1
$me = $PID
Get-CimInstance Win32_Process -Filter "Name='powershell.exe'" |
    Where-Object { $_.ProcessId -ne $me -and $_.CommandLine -match 'experiments\\win_(7|8)\d_' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue; "stopped script pid $($_.ProcessId)" }
foreach ($n in 'rag', 'rag-desktop', 'llama-server') {
    Get-Process $n -ErrorAction SilentlyContinue | Where-Object { $_.Path -like 'C:\Users\k0937\Desktop\rag\*' } |
        ForEach-Object { Stop-Process -Id $_.Id -Force; "stopped $n pid $($_.Id)" }
}
# win_73 開的臨時容器（不動開發用的 rag-rust-box）
if (Get-Command docker -ErrorAction SilentlyContinue) {
    docker ps -q --filter ancestor=rag-rust | ForEach-Object {
        $name = docker inspect -f '{{.Name}}' $_
        if ($name -ne '/rag-rust-box') { docker kill $_ | Out-Null; "stopped container $name" }
    }
}
'all measurement jobs stopped'
