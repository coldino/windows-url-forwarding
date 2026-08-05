$regPath = "HKEY_CURRENT_USER\Software\Classes\url-ferry"
reg add "$regPath" /ve /d "URL Ferry Handler" /f
reg add "$regPath\DefaultIcon" /ve /d "D:\Work\Desktop\url-forwarding\target\release\url-ferry-sender.exe,0" /f
reg add "$regPath\shell\open\command" /ve /d "`"D:\Work\Desktop\url-forwarding\target\release\url-ferry-sender.exe`" `"%%1`"" /f

# Then point http/https to this ProgID
reg add "HKEY_CURRENT_USER\Software\Classes\http" /v "" /d "url-ferry" /f
reg add "HKEY_CURRENT_USER\Software\Classes\https" /v "" /d "url-ferry" /f
