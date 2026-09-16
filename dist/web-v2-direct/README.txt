rustdesk-web-v2-direct
======================
Built from weiwan-gmail/rustdesk develop after PR #32
  commit:  f222cd33ea1c395d8135283dde6244f4e64c0c81
  merge:   Merge pull request #32 (web Map-mode keyboard for Windows lock screen)
  date:    2026-09-16
  script:  deploy/v2/web-direct/build.sh --all
  Flutter: 3.24.5

Linux (amd64):
  unzip rustdesk-web-v2-direct-linux-amd64-f222cd33e.zip
  chmod +x rustdesk-web-v2-direct
  ./rustdesk-web-v2-direct
  # open http://localhost:8081
  # in "Remote ID" enter the controlled Windows IP (e.g. 192.168.1.50 or 192.168.1.50:21118)

Windows (amd64):
  unzip rustdesk-web-v2-direct-windows-amd64-f222cd33e.zip
  rustdesk-web-v2-direct.exe
  # same: http://localhost:8081  then IP of the controlled client

Controlled client: enable Direct IP access (default port 21118).
