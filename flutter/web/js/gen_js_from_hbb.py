#!/usr/bin/env python3

import re
import os
import glob
from tabnanny import check

def pad_start(s, n, c = ' '):
   if len(s) >= n:
      return s
   return c * (n - len(s)) + s

def safe_unicode(s):
   res = ""
   for c in s:
      res += r"\u{}".format(pad_start(hex(ord(c))[2:], 4, '0'))
   return res

def main():
   print('export const LANGS = {')
   for fn in glob.glob('../../../src/lang/*'):
      lang = os.path.basename(fn)[:-3]
      if lang == 'template': continue
      print('  %s: {'%lang)
      for ln in open(fn, encoding='utf-8'):
         ln = ln.strip()
         if ln.startswith('("'):
            toks = ln.split('", "')
            assert(len(toks) == 2)
            a = toks[0][2:]
            b = toks[1][:-3]
            print('    "%s": "%s",'%(safe_unicode(a), safe_unicode(b)))
      print('  },')
   print('}')
   KEY_MAP = ['', False]
   for ln in open('../../../src/client.rs', encoding='utf-8'):
      ln = ln.strip()
      if 'KEY_MAP' in ln:
         KEY_MAP[1] = True
         continue
      if '.collect' in ln and KEY_MAP[1]:
         KEY_MAP[1] = False
         continue
      if KEY_MAP[1] and ln.startswith('('):
         ln = removeComment(ln)
         toks = ln.split('", Key::')
         assert(len(toks) == 2)
         a = toks[0][2:]
         b = toks[1].replace('ControlKey(ControlKey::', '').replace("Chr('", '').replace("' as _)),", '').replace(')),', '')
         KEY_MAP[0] += '  "%s": "%s",\n'%(a, b)
   print()
   # Hand-written port of check_if_retry() in ../../../src/client.rs. The v1
   # generator rewrote the Rust body textually, which broke once the function
   # gained a use_ws() call; the web client always connects over websocket, so
   # use_ws() is constant true here.
   print('export function checkIfRetry(msgtype: string, title: string, text: string,  retry_for_relay: boolean) {')
   print('  const t = text.toLowerCase();')
   print('  return msgtype == "error"')
   print('    && title == "Connection Error"')
   print('    && (((t.indexOf("10054") >= 0 || t.indexOf("104") >= 0) && retry_for_relay)')
   print('      || (t.indexOf("offline") < 0')
   print('        && t.indexOf("not exist") < 0')
   print('        && (t.indexOf("handshake") < 0 || t.indexOf("connection reset without closing handshake") >= 0)')
   print('        && t.indexOf("failed") < 0')
   print('        && t.indexOf("resolve") < 0')
   print('        && t.indexOf("mismatch") < 0')
   print('        && t.indexOf("manually") < 0')
   print('        && t.indexOf("restricted") < 0')
   print('        && t.indexOf("not allowed") < 0));')
   print('}')
   print()
   print('export const KEY_MAP: any = {')
   print(KEY_MAP[0])
   print('}')
   for ln in open('../../../Cargo.toml', encoding='utf-8'):
      if ln.startswith('version ='):
         print('export const ' + ln)


def removeComment(ln):
   return re.sub('\s+\/\/.*$', '', ln)

main()
