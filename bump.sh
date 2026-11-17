#!/bin/sh
# Đặt phiên bản theo ngày cập nhật (hiển thị DDMM.YY).
#   ./bump.sh               ngày hôm nay, lần 1 trong ngày        -> 1511.26
#   ./bump.sh 15 11 26      chọn ngày                             -> 1511.26
#   ./bump.sh 15 11 26 2    lần cập nhật thứ 2 trong cùng ngày    -> 1511.26.2
# "version" của Cargo phải là DD.MM.YY (mỗi số <= 255, không số 0 đứng đầu) vì cargo-apk suy versionCode từ đó.
set -e
if [ $# -ge 3 ]; then D=$1; M=$2; Y=$3; R=${4:-1}; else D=$(date +%d); M=$(date +%m); Y=$(date +%y); R=${1:-1}; fi
if [ "$R" = "1" ]; then V="$D$M.$Y"; CV="$((10#$D)).$((10#$M)).$((10#$Y))"; else V="$D$M.$Y.$R"; CV="$((10#$D)).$((10#$M)).$((10#$Y))-$R"; fi
CODE=$(printf "%s%s%s%02d" "$Y" "$M" "$D" "$R")
sed -i "0,/^version = .*/s//version = \"$CV\"/" Cargo.toml
sed -i "s/^version_name = .*/version_name = \"$V\"/; s/^version_code = .*/version_code = $CODE/" Cargo.toml
echo "Hiển thị: $V | Cargo: $CV | versionCode: $CODE. Nhớ ghi vào CHANGELOG.md"
