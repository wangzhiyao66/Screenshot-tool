#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""把调研报告 Markdown 转成带样式的 HTML（浅色主题，适合阅读与打印）。"""

import pathlib
import markdown

BASE = pathlib.Path(__file__).parent
SRC = BASE / "截图工具竞品与技术调研报告.md"
DST = BASE / "截图工具竞品与技术调研报告.html"

CSS = """
:root{
  --bg:#ffffff; --panel:#f7f8fa; --ink:#1f2328; --ink-soft:#57606a;
  --line:#e3e6ea; --accent:#c2410c; --accent-soft:#fdf3ee; --code-bg:#f6f8fa;
}
*{box-sizing:border-box}
html{-webkit-text-size-adjust:100%}
body{
  margin:0; background:var(--bg); color:var(--ink);
  font-family:-apple-system,BlinkMacSystemFont,"Segoe UI","PingFang SC","Hiragino Sans GB","Microsoft YaHei",sans-serif;
  font-size:15.5px; line-height:1.85; letter-spacing:.01em;
}
.wrap{max-width:920px; margin:0 auto; padding:56px 32px 96px}
h1{
  font-size:30px; line-height:1.35; font-weight:700; letter-spacing:-.01em;
  margin:0 0 28px; padding-bottom:20px; border-bottom:2px solid var(--ink);
}
h2{
  font-size:21px; font-weight:700; margin:56px 0 18px; padding:0 0 10px;
  border-bottom:1px solid var(--line);
}
h3{font-size:17px; font-weight:650; margin:34px 0 12px; color:var(--ink)}
p{margin:0 0 14px}
strong{font-weight:650}
a{color:var(--accent); text-decoration:none; border-bottom:1px solid rgba(194,65,12,.3)}
a:hover{border-bottom-color:var(--accent)}
ul,ol{margin:0 0 16px; padding-left:24px}
li{margin:5px 0}
blockquote{
  margin:20px 0; padding:16px 20px; background:var(--accent-soft);
  border-left:3px solid var(--accent); border-radius:0 6px 6px 0; color:#3f2a20;
}
blockquote p{margin:0}
blockquote p + p{margin-top:10px}
hr{border:0; border-top:1px solid var(--line); margin:44px 0}
code{
  font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace; font-size:.88em;
  background:var(--code-bg); padding:2px 6px; border-radius:4px; border:1px solid var(--line);
}
pre{
  background:var(--code-bg); border:1px solid var(--line); border-radius:8px;
  padding:16px 18px; overflow-x:auto; margin:18px 0;
}
pre code{background:none; border:0; padding:0; font-size:13px; line-height:1.7}
table{
  width:100%; border-collapse:collapse; margin:22px 0; font-size:13.5px;
  display:block; overflow-x:auto; white-space:nowrap;
}
thead th{
  background:var(--panel); font-weight:650; text-align:left;
  padding:10px 12px; border-bottom:2px solid var(--line); border-top:1px solid var(--line);
  white-space:nowrap;
}
tbody td{padding:9px 12px; border-bottom:1px solid var(--line); vertical-align:top}
tbody tr:hover{background:#fcfcfd}
tbody td:nth-child(n+2), thead th:nth-child(n+2){text-align:center}
tbody td:first-child{font-weight:600; white-space:nowrap; text-align:left}
h2 + p > em,blockquote > p > em{color:var(--ink-soft)}
@media (max-width:640px){
  .wrap{padding:32px 18px 64px}
  h1{font-size:24px}
  body{font-size:15px}
}
@media print{
  body{font-size:11pt}
  .wrap{max-width:none; padding:0}
  h2{page-break-after:avoid}
  table{page-break-inside:avoid}
  a{border:0; color:#000}
}
"""

HTML = """<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>{css}</style>
</head>
<body>
<div class="wrap">
{body}
</div>
</body>
</html>
"""


def main() -> None:
    text = SRC.read_text(encoding="utf-8")
    title = text.lstrip("#").splitlines()[0].strip()
    md = markdown.Markdown(
        extensions=["tables", "fenced_code", "sane_lists", "attr_list", "toc"],
        extension_configs={"toc": {"toc_depth": "2-3"}},
    )
    body = md.convert(text)
    DST.write_text(HTML.format(title=title, css=CSS, body=body), encoding="utf-8")
    print(f"ok -> {DST}")


if __name__ == "__main__":
    main()
