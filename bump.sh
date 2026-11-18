#!/bin/sh
# Đặt phiên bản theo ngày cập nhật (hiển thị DDMM.YY).
#   ./bump.sh               ngày hôm nay, lần 1 trong ngày        -> 1511.26
#   ./bump.sh 15 11 26      chọn ngày                             -> 1511.26
#   ./bump.sh 15 11 26 2    lần cập nhật thứ 2 trong cùng ngày    -> 1511.26.2
# Cargo "version" = YY.MM.DD (mỗi số <= 255, không 0 đứng đầu): cargo-apk suy versionName/versionCode từ đây
# nên versionCode tăng đúng theo ngày. Số hiển thị (tên APK, tiêu đề app) nằm ở display_version.
set -e
if [ $# -ge 3 ]; then D=$1; M=$2; Y=$3; R=${4:-1}; else D=$(date +%d); M=$(date +%m); Y=$(date +%y); R=${1:-1}; fi
if [ "$R" = "1" ]; then V="$D$M.$Y"; CV="$((10#$Y)).$((10#$M)).$((10#$D))"; else V="$D$M.$Y.$R"; CV="$((10#$Y)).$((10#$M)).$((10#$D))-$R"; fi
sed -i "0,/^version = .*/s//version = \"$CV\"/" Cargo.toml
sed -i "s/^display_version = .*/display_version = \"$V\"/" Cargo.toml
echo "Hiển thị: $V | Cargo: $CV. Nhớ ghi vào CHANGELOG.md"
