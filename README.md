# CSan (Rust) – build APK bằng GitHub Actions

## Cách dùng
1. Tạo repo mới trên GitHub, giải nén zip này và đẩy toàn bộ lên nhánh `main`
   (nhớ cả thư mục ẩn `.github`).
2. Vào tab **Actions** → workflow **Build APK** tự chạy (hoặc bấm *Run workflow*).
   Lần đầu ~15–25 phút (cài cargo-apk + biên dịch egui), các lần sau nhanh hơn nhờ cache.
3. Khi xong, mở lần chạy đó → mục **Artifacts** → tải `CSan-apk` → giải nén → cài `csan.apk`.

Gói ứng dụng: `zanq.fun.camera`. APK ký bằng khoá tạm sinh mỗi lần build, nên mỗi bản có chữ ký khác nhau
(phải gỡ bản cũ trước khi cài bản mới). Muốn chữ ký cố định: tạo secrets `KEYSTORE_BASE64`
(`base64 -w0 release.jks`) và `KEYSTORE_PASSWORD` trong Settings → Secrets → Actions.

## Phiên bản
Ghi theo ngày cập nhật `DDMM.YY` (14/11/2026 → `1411.26`). Chạy `./bump.sh` để đặt phiên bản theo ngày hôm nay
(hoặc `./bump.sh 14 11 26`), rồi ghi vào `CHANGELOG.md`. APK tải về có tên `CSan-1411.26.apk`.

## Đã có (1511.26)
Ảnh: JPEG/PNG/WebP/HEIC, HDR 3 ảnh, chụp đêm, flash, hẹn giờ, zoom, chạm lấy nét, lưới, thước thăng bằng, tỉ lệ 4:3/16:9, khử nhiễu qua server.
Live: ảnh động (Motion Photo). Video: H.264/HEVC + âm thanh (mic chính), 720p/1080p/4K, EIS/OIS, ổn định qua server.
TikTok: chọn/cắt nhạc, đổi tốc độ, ghép nhạc.

## Chưa có / không port
- Camera API 1 và CameraX: cần thư viện Java/AndroidX nên không làm được trong APK Rust thuần. Đã có chế độ tương thích tự động.
- Live định dạng HEIC (HeifXmp), màn hình MoreSettings.

## Lưu ý
- Code chưa được biên dịch thử khi viết. Nếu Actions báo lỗi, gửi log bước **Build** để sửa.
- Một số mã khoá metadata Camera2 viết theo header NDK; máy nào không nhận thì tính năng đó bị bỏ qua.
- Live cần máy cho phép 3 luồng cùng lúc (xem trước + JPEG + mã hóa); nếu không, chế độ LIVE báo lỗi.
- TikTok/nhạc cần cấp quyền đọc âm thanh; bấm "Làm mới" trong danh sách nhạc sau khi cấp quyền.
- Ảnh/video lưu ở `Android/data/zanq.fun.camera/files/` (chưa vào thư viện ảnh).
