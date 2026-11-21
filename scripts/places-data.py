#!/usr/bin/env python3
"""Makes data/places.tsv.gz, the towns the `places` feature names a position by (record 0010), from
GeoNames' files, which it doesn't download: get them from https://download.geonames.org/export/dump/
into one folder, then

    python3 -I scripts/places-data.py <folder> data/places.tsv.gz

The files are `cities1000.zip`, every town of over 1,000 people, `admin1CodesASCII.txt`, the
regions' names, and `countryInfo.txt`, the countries'. GeoNames' data is under CC BY 4.0; this
keeps only what a lookup needs, and leaves out neighbourhoods (feature code PPLX). The output is the
same for the same files: gzip without a time, lines in GeoNames' order.

The output's lines, tab-separated, after a first line naming its source:
    C <country code> <name>
    R <country code>.<region code> <name>
    T <name> <latitude> <longitude> <country code> <region code> <people>
"""
import gzip
import hashlib
import io
import os
import sys
import zipfile


def main(folder: str, out: str) -> None:
    files = {name: open(os.path.join(folder, name), "rb").read() for name in ("cities1000.zip", "admin1CodesASCII.txt", "countryInfo.txt")}
    lines = ["# GeoNames, geonames.org, CC BY 4.0: " + ", ".join(f"{name} sha256 {hashlib.sha256(data).hexdigest()[:16]}" for name, data in files.items())]

    for row in files["countryInfo.txt"].decode("utf-8").splitlines():
        if row and not row.startswith("#"):
            columns = row.split("\t")
            lines.append(f"C\t{columns[0]}\t{columns[4]}")
    for row in files["admin1CodesASCII.txt"].decode("utf-8").splitlines():
        columns = row.split("\t")
        if len(columns) >= 2:
            lines.append(f"R\t{columns[0]}\t{columns[1]}")

    towns = zipfile.ZipFile(io.BytesIO(files["cities1000.zip"])).read("cities1000.txt").decode("utf-8")
    kept = left = 0
    for row in towns.splitlines():
        c = row.split("\t")
        name, latitude, longitude, feature, country, region, people = c[1], float(c[4]), float(c[5]), c[7], c[8], c[10], int(c[14] or 0)
        if feature == "PPLX":
            left += 1
            continue
        lines.append(f"T\t{name}\t{latitude:.4f}\t{longitude:.4f}\t{country}\t{region}\t{people}")
        kept += 1

    data = ("\n".join(lines) + "\n").encode("utf-8")
    with open(out, "wb") as f:
        f.write(gzip.compress(data, compresslevel=9, mtime=0))
    print(f"{out}: {kept} towns, {left} neighbourhoods left out, {os.path.getsize(out) / 1e6:.2f} MB")
    print(lines[0])


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
