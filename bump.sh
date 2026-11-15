#!/bin/sh
# Đặt phiên bản theo ngày cập nhật (DDMM.YY).
#   ./bump.sh               ngày hôm nay, lần 1 trong ngày        -> 1411.26
#   ./bump.sh 14 11 26      chọn ngày                             -> 1411.26
#   ./bump.sh 14 11 26 2    lần cập nhật thứ 2 trong cùng ngày    -> 1411.26.2
set -e
if [ $# -ge 3 ]; then D=$1; M=$2; Y=$3; R=${4:-1}; else D=$(date +%d); M=$(date +%m); Y=$(date +%y); R=${1:-1}; fi
if [ "$R" = "1" ]; then V="$D$M.$Y"; CV="$V.0"; else V="$D$M.$Y.$R"; CV="$V"; fi
CODE=$(printf "%s%s%s%02d" "$Y" "$M" "$D" "$R")
sed -i "0,/^version = .*/s//version = \"$CV\"/" Cargo.toml
sed -i "s/^version_name = .*/version_name = \"$V\"/; s/^version_code = .*/version_code = $CODE/" Cargo.toml
echo "Phiên bản: $V (versionCode $CODE). Nhớ ghi vào CHANGELOG.md"
