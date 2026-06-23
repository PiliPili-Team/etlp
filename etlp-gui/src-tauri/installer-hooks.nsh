; Remove .lproj directories left by versions that bundled them at the
; install root. These are macOS NSBundle locale folders that were
; mistakenly included in Windows builds and are no longer shipped.
!macro customInstall
  RMDir /r "$INSTDIR\zh-Hans.lproj"
  RMDir /r "$INSTDIR\zh-Hant.lproj"
  RMDir /r "$INSTDIR\ja.lproj"
  RMDir /r "$INSTDIR\ko.lproj"
  RMDir /r "$INSTDIR\de.lproj"
  RMDir /r "$INSTDIR\it.lproj"
  RMDir /r "$INSTDIR\fr.lproj"
  RMDir /r "$INSTDIR\ar.lproj"
  RMDir /r "$INSTDIR\es.lproj"
  RMDir /r "$INSTDIR\ru.lproj"
  RMDir /r "$INSTDIR\pt.lproj"
  RMDir /r "$INSTDIR\sk.lproj"
  RMDir /r "$INSTDIR\uk.lproj"
  RMDir /r "$INSTDIR\sr.lproj"
  RMDir /r "$INSTDIR\tr.lproj"
  RMDir /r "$INSTDIR\he.lproj"
  RMDir /r "$INSTDIR\th.lproj"
  RMDir /r "$INSTDIR\pl.lproj"
  RMDir /r "$INSTDIR\id.lproj"
!macroend
