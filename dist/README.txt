Relay — a small, fast REST client
==================================

Relay lets you build and send HTTP requests (like Postman or Insomnia),
with collections, history, environments, code generation and cURL import.
It runs entirely on your machine — nothing is uploaded anywhere, and your
saved requests live in your own browser's local storage.

Pick the file for your computer
-------------------------------
  Relay-Windows.exe            Windows (10/11)
  Relay-macOS-AppleSilicon     Mac with Apple Silicon (M1/M2/M3/M4)
  Relay-macOS-Intel            Mac with an Intel processor

How to run — Windows
--------------------
1. Double-click Relay-Windows.exe.
2. If Windows SmartScreen shows "Windows protected your PC", click
   "More info" then "Run anyway". (The app is unsigned — that warning
   appears for any app that isn't from a registered publisher.)
3. A console window opens and your browser pops up with Relay.
   Keep the console window open while you use it; close it to quit.

How to run — macOS
------------------
1. Open Terminal in the folder you downloaded the file to, then run:
     chmod +x Relay-macOS-AppleSilicon     (or Relay-macOS-Intel)
2. First launch: right-click the file in Finder and choose "Open",
   then confirm — or run it from Terminal:
     ./Relay-macOS-AppleSilicon
   If macOS still blocks it ("cannot be opened because it is from an
   unidentified developer"), run:
     xattr -d com.apple.quarantine Relay-macOS-AppleSilicon
   and try again.
3. Your browser opens with Relay. Keep the terminal window open while
   you use it; press Ctrl+C there to quit.

Good to know
------------
- The green "Native engine" badge in the bottom-left corner means requests
  are sent by the Relay app itself, with every response header visible.
- Relay listens only on 127.0.0.1 (your own machine) and its request
  engine is protected by a per-session token — other devices and websites
  cannot use it.
- Your requests, collections and history are saved per browser on each
  machine. Use Export/Import in the sidebar to move them between machines
  or share them with someone.
