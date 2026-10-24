"""Write a plain-text license as RTF, for the license page of the MSI installer.

Usage: python packaging/windows/license_rtf.py LICENSE License.rtf
"""

import sys


def to_rtf(text: str) -> str:
    out = [r"{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0\fmodern Consolas;}}\f0\fs16 "]
    for line in text.splitlines():
        escaped = []
        for ch in line:
            if ch in "\\{}":
                escaped.append("\\" + ch)
            elif ord(ch) < 128:
                escaped.append(ch)
            else:
                code = ord(ch)
                escaped.append(f"\\u{code if code < 32768 else code - 65536}?")
        out.append("".join(escaped) + "\\par\n")
    out.append("}\n")
    return "".join(out)


def main() -> None:
    source, target = sys.argv[1], sys.argv[2]
    with open(source, encoding="utf-8") as f:
        text = f.read()
    with open(target, "w", encoding="ascii", newline="\n") as f:
        f.write(to_rtf(text))


if __name__ == "__main__":
    main()
