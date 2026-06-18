' run-silent.vbs
' Launch etlp.exe without a console window on Windows.
'
' Place this file in the Windows Startup folder for autostart:
'   %APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\
'
' Or run directly: wscript run-silent.vbs [--data-dir DIR]
'
' Auto-generated template — edit BINARY_PATH to match your installation.

Option Explicit

Const BINARY_PATH = "C:\Program Files\etlp\etlp.exe"

Dim oWS, sArgs, i
Set oWS = WScript.CreateObject("WScript.Shell")

' Pass through any arguments supplied to this script.
sArgs = ""
For i = 0 To WScript.Arguments.Count - 1
    Dim arg : arg = WScript.Arguments(i)
    ' Wrap in quotes if the argument contains spaces.
    If InStr(arg, " ") > 0 Then
        sArgs = sArgs & " """ & arg & """"
    Else
        sArgs = sArgs & " " & arg
    End If
Next

' WindowStyle 0 = hidden (no console window).
oWS.Run """" & BINARY_PATH & """" & sArgs, 0, False

Set oWS = Nothing
