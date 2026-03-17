#!/usr/bin/env python3
"""tc2辞書とSKK辞書をマージして交ぜ書き辞書を生成する。

共通見出し語はSKK(rtry)の候補を優先し、tc2にしかない候補を末尾に追加。
tc2固有・rtry固有の見出し語はそのまま維持。
"""

import sys
from collections import OrderedDict


def parse_dict(path: str) -> OrderedDict[str, list[str]]:
    """SKK形式の辞書を読み込む。"""
    entries: OrderedDict[str, list[str]] = OrderedDict()
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith(";"):
                continue
            parts = line.split(" ", 1)
            if len(parts) != 2:
                continue
            reading = parts[0]
            candidates_str = parts[1].strip().strip("/")
            candidates = [c for c in candidates_str.split("/") if c]
            if reading and candidates:
                entries[reading] = candidates
    return entries


def merge_candidates(base: list[str], extra: list[str]) -> list[str]:
    """base を優先しつつ、extra にしかない候補を末尾に追加。
    アノテーション(;以降)を除いた候補文字列で重複判定。"""
    seen = set()
    result = []
    for c in base:
        word = c.split(";")[0]
        if word not in seen:
            seen.add(word)
            result.append(c)
    for c in extra:
        word = c.split(";")[0]
        if word not in seen:
            seen.add(word)
            result.append(c)
    return result


def main():
    if len(sys.argv) != 4:
        print(f"Usage: {sys.argv[0]} <rtry_dict> <tc2_dict> <output>", file=sys.stderr)
        sys.exit(1)

    rtry_path, tc2_path, output_path = sys.argv[1], sys.argv[2], sys.argv[3]

    print(f"Loading rtry dict: {rtry_path}")
    rtry = parse_dict(rtry_path)
    print(f"  {len(rtry)} entries")

    print(f"Loading tc2 dict: {tc2_path}")
    tc2 = parse_dict(tc2_path)
    print(f"  {len(tc2)} entries")

    # マージ
    merged: OrderedDict[str, list[str]] = OrderedDict()

    # rtryのエントリをベースに
    for reading, candidates in rtry.items():
        if reading in tc2:
            merged[reading] = merge_candidates(candidates, tc2[reading])
        else:
            merged[reading] = candidates

    # tc2固有のエントリを追加
    tc2_only = 0
    for reading, candidates in tc2.items():
        if reading not in merged:
            merged[reading] = candidates
            tc2_only += 1

    # ソート（日本語辞書順）
    sorted_entries = sorted(merged.items(), key=lambda x: x[0])

    # 統計
    rtry_only = sum(1 for r in rtry if r not in tc2)
    common = sum(1 for r in rtry if r in tc2)
    print(f"\nMerge result:")
    print(f"  rtry only:  {rtry_only}")
    print(f"  tc2 only:   {tc2_only}")
    print(f"  common:     {common}")
    print(f"  total:      {len(sorted_entries)}")

    # 書き出し
    with open(output_path, "w", encoding="utf-8") as f:
        f.write(";; mazegaki.dic - Merged mazegaki dictionary\n")
        f.write(";;\n")
        f.write(";; This dictionary is merged from two sources:\n")
        f.write(";;\n")
        f.write(";; 1. SKK-JISYO.L by the SKK Development Team\n")
        f.write(";;    License: GPL-2.0-or-later\n")
        f.write(";;    See: https://github.com/skk-dev/dict\n")
        f.write(";;\n")
        f.write(";; 2. tc2 mazegaki dictionary (T-Code package for Emacs)\n")
        f.write(";;    License: GPL-2.0\n")
        f.write(";;    Source data derived from Wnn pubdic (kihon.u, tankan.u)\n")
        f.write(";;    See: https://github.com/kanchoku/tc\n")
        f.write(";;\n")
        f.write(";; Format: reading /candidate1/candidate2/.../\n")
        f.write(";;\n")
        for reading, candidates in sorted_entries:
            f.write(f"{reading} /{'/'.join(candidates)}/\n")

    print(f"\nWritten to: {output_path}")


if __name__ == "__main__":
    main()
