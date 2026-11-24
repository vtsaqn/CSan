# CSar (Rust) – build APK bằng GitHub Actions

## Cách dùng
1. Tạo repo mới trên GitHub, giải nén zip này và đẩy toàn bộ lên nhánh `main`
   (nhớ cả thư mục ẩn `.github`).
2. Vào tab **Actions** → workflow **Build APK** tự chạy (hoặc bấm *Run workflow*).
   Lần đầu ~15–25 phút (cài cargo-apk + biên dịch egui), các lần sau nhanh hơn nhờ cache.
3. Khi xong, mở lần chạy đó → mục **Artifacts** → tải `CSar-apk` → giải nén → cài `csan.apk`.

Gói ứng dụng: `zanq.fun.csar`. APK ký bằng khoá tạm sinh mỗi lần build, nên mỗi bản có chữ ký khác nhau
(phải gỡ bản cũ trước khi cài bản mới). Muốn chữ ký cố định: tạo secrets `KEYSTORE_BASE64`
(`base64 -w0 release.jks`) và `KEYSTORE_PASSWORD` trong Settings → Secrets → Actions.

## Phiên bản
Ghi theo ngày cập nhật `DDMM.YY` (14/11/2026 → `1411.26`; lần thứ 2 trong ngày → `1411.26.2`). Trong Cargo.toml, `version` dạng YY.MM.DD (cargo-apk tự suy versionCode từ đây, mỗi số <= 255) còn `display_version` là số hiển thị. Chạy `./bump.sh` để cập nhật cả hai. Chạy `./bump.sh` để đặt phiên bản theo ngày hôm nay
(hoặc `./bump.sh 14 11 26`), rồi ghi vào `CHANGELOG.md`. APK tải về có tên `CSan-1411.26.apk`.

## Đã có (1511.26)
Ảnh: JPEG/PNG/WebP/HEIC, HDR 3 ảnh, chụp đêm, flash, hẹn giờ, zoom, chạm lấy nét, lưới, thước thăng bằng, tỉ lệ 4:3/16:9, khử nhiễu qua server.
Live: ảnh động (Motion Photo). Video: H.264/HEVC + âm thanh (mic chính), 720p/1080p/4K, EIS/OIS, ổn định qua server.
TikTok: chọn/cắt nhạc, đổi tốc độ, ghép nhạc.

## Chưa có / không port
- CameraX: bỏ. Camera API 1: để sau (hiện có chế độ tương thích tự động khi Camera2 lỗi).
- Live định dạng HEIC (HeifXmp), màn hình MoreSettings.

## Lưu ý
- Code chưa được biên dịch thử khi viết. Nếu Actions báo lỗi, gửi log bước **Build** để sửa.
- Một số mã khoá metadata Camera2 viết theo header NDK; máy nào không nhận thì tính năng đó bị bỏ qua.
- Live cần máy cho phép 3 luồng cùng lúc (xem trước + JPEG + mã hóa); nếu không, chế độ LIVE báo lỗi.
- TikTok/nhạc cần cấp quyền đọc âm thanh; bấm "Làm mới" trong danh sách nhạc sau khi cấp quyền.
- Ảnh/video lưu ở thư viện máy: DCIM/Csan. Tệp tạm và config.txt nằm ở `Android/data/zanq.fun.csar/files/`.

## Livestream (chế độ STREAM)
Giao thức: `rtmp://host[:1935]/app` và `rtmps://host[:443]/app`; khoá luồng nhập riêng hoặc nối sau URL (`.../app/KEY`).
H.265 dùng Enhanced RTMP (FourCC `hvc1`) nên máy chủ phải hỗ trợ (YouTube, Twitch Enhanced Broadcasting…); nginx-rtmp bản gốc thì dùng H.264.
Cần để màn hình sáng và app ở nền trước khi phát. WHIP/WebRTC chưa có.

## Plugin ổn định video — giao thức CSAR-STB v1
Plugin là app riêng, mở cổng TCP `127.0.0.1:<cổng>` (mặc định 7788) khi đang chạy. Mọi số nhiều byte theo big-endian.
1. App → plugin (khi bắt đầu phát): `"CSARSTB1"` + `w`(u16) + `h`(u16) + `fps`(u8) + `mức ổn định`(u8, 0–100) + `tỉ lệ khung giữ lại`(u8, %) + `cửa sổ làm mượt`(u16, số khung).
2. Plugin → app: `"CSAROK1"` (7 byte).
3. Mỗi khung: app → plugin `len`(u32) + NV12 đóng gói chặt (`w*h*3/2` byte, đã xoay đúng hướng); plugin → app `len`(u32) + NV12 cùng kích thước đã ổn định. `len = 0` nghĩa là dùng nguyên khung gốc.
Mỗi khung chờ tối đa 300 ms; quá hạn hoặc lỗi thì app tắt ổn định và phát tiếp không ổn định.
