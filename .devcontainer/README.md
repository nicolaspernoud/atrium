# Using the dev container

## Connect to android emulator on windows

On the windows host :
```powershell
# As admin
New-NetFirewallRule -DisplayName "ADB TCP" -Direction Inbound -LocalPort 5555 -Protocol TCP -Action Allow
netsh interface portproxy add v4tov4 listenport=5555 listenaddress=0.0.0.0 connectport=5555 connectaddress=127.0.0.1
# As user or admin
.\adb.exe kill-server
.\adb.exe tcpip 5555
```

On the WLS2 Linux Host, get the Windows Host IP
```bash
/sbin/ip route | awk '/default/ { print $3 }'
```

In the dev container :
```bash
adb connect <Windows Host IP>
adb devices
```