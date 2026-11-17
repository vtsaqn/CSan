# Lịch sử cập nhật
Phiên bản ghi theo ngày cập nhật: `DDMM.YY` (14/11/2026 → `1411.26`).

## 1511.26.2 — 15/11/2026
- Sửa lỗi build "Invalid semver": cargo-apk chỉ nhận phiên bản Cargo có mỗi số <= 255, nên `version` trong Cargo.toml đổi sang dạng DD.MM.YY (15.11.26-2); phiên bản hiển thị 1511.26.2 nằm ở `version_name`. App và tên APK vẫn dùng 1511.26.2.

## 1511.26.1 — 15/11/2026
- Sửa workflow: ghim build-tools 34.0.0 + NDK mặc định của runner, bỏ biến ANDROID_SDK_ROOT cũ, in thông tin SDK/NDK để chẩn đoán lỗi "Invalid semver" của cargo-apk.

## 1511.26 — 15/11/2026
- Live (ảnh động): bộ mã hóa chạy nền giữ ~6 giây gần nhất, cắt clip ±1,5 giây quanh lúc bấm chụp, ghép thành Motion Photo (JPEG + XMP Google/OPlus + MP4).
- TikTok: chọn nhạc trên máy, cắt đoạn nhạc, nghe thử, thời lượng 15/30/60 giây, tốc độ 3x/2x/1x/0,5x/0,25x; ghép nhạc (giải mã → AAC) và đổi tốc độ bằng co/giãn mốc thời gian (như TikMux).
- HEIC: mã hóa HEVC bằng phần cứng rồi tự đóng gói HEIF; chỉ dùng khi nhỏ hơn JPEG rõ rệt, nếu không tự lưu JPEG.
- Thước thăng bằng (gia tốc kế): đường ngang khi cầm dọc, bong bóng khi đặt phẳng.
- Micro: luôn dùng mic chính của máy (đã bỏ định tuyến mic Bluetooth).
- Camera API 1 / CameraX: không port (cần thư viện Java/AndroidX). Thay bằng "chế độ tương thích": nếu mở camera lỗi, tự thử lại với kích thước nhỏ hơn.
- Chưa có: Live định dạng HEIC (HeifXmp), màn hình MoreSettings.

## 1411.26.1 — 14/11/2026
- Quay video (H.264/HEVC) kèm âm thanh AAC, 720p/1080p/4K, bitrate, chống rung EIS/OIS, cho giảm fps khi thiếu sáng.
- Ổn định video qua server (StabClient) sau khi quay.
- HDR ghép 3 ảnh (HdrFuse port sang Rust): thủ công/tự động theo cảnh.
- Chụp đêm (tăng EV + khử nhiễu chất lượng cao), flash tự động/bật, đèn pin, hẹn giờ 3/10 giây.
- Zoom (chụm 2 ngón + nút), chạm lấy nét/đo sáng, lưới 3x3, tỉ lệ 4:3 / 16:9.
- Lưu ảnh JPEG / PNG / WebP, chất lượng JPEG chỉnh được.
- Màn hình Cài đặt đầy đủ hơn. versionCode đổi sang dạng YYMMDDNN.
- Chưa có: Live (ảnh động), TikTok (nhạc/cắt), HEIC, thước thăng bằng, Camera API 1/CameraX, mic bluetooth.

## 1411.26 — 14/11/2026
- Khởi tạo dự án Rust (egui + NDK Camera2), build APK bằng GitHub Actions.
- Xem trước camera, chụp JPEG, đổi camera trước/sau.
- Gửi ảnh lên server khử nhiễu, cài đặt lưu tệp.
- Chưa có: quay video, HDR, HEIC, zoom/flash, EIS/OIS, giao diện iOS, MoreSettings.
