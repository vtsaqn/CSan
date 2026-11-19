# Lịch sử cập nhật
Phiên bản ghi theo ngày cập nhật: `DDMM.YY` (14/11/2026 → `1411.26`).

## 1911.26 — 19/11/2026
- Đổi mã gói từ `zanq.fun.camera` sang `zanq.fun.csar` để cài song song với app Java gốc (hai app riêng, không ghi đè nhau). Thư mục dữ liệu mới: `Android/data/zanq.fun.csar/files/`; cài đặt của bản trước không tự chuyển sang. Ảnh/video vẫn lưu ở DCIM/Csan.

## 1811.26.5 — 18/11/2026
- Đổi tên ứng dụng thành CSar: nhãn dưới icon, tiêu đề trong app, tên tệp APK (CSar-<phiên bản>.apk) và tên artifact trên GitHub. Mã gói `zanq.fun.camera` và thư mục DCIM/Csan giữ nguyên để không mất dữ liệu cũ.

## 1811.26.4 — 18/11/2026
- Giao diện đồng đều ở mọi chế độ (Ảnh/Live/Video/TikTok): thanh trên luôn một hàng (tên + Lưới + Cài đặt); thanh dưới có chiều cao cố định, bốn hàng luôn ở cùng vị trí: tuỳ chọn → zoom → chế độ → nút chụp.
- Hàng tuỳ chọn đổi theo chế độ, mỗi nút bấm để chuyển giá trị: Ảnh (Flash, HDR, Hẹn giờ, Tỉ lệ); Live (Flash, Hẹn giờ, Tỉ lệ); Video (Chống rung, Độ phân giải, Tỉ lệ); TikTok (Nhạc, Tốc độ, Thời lượng, Tỉ lệ).
- Thông báo và thời gian quay vẽ đè lên đầu ảnh xem trước (tự ẩn sau 6–12 giây), không còn chiếm chỗ trong bố cục.
- Tỉ lệ 16:9: hàng tuỳ chọn ẩn, bấm "⋯ Tùy chọn" để mở.

## 1811.26.3 — 18/11/2026
- Sửa vùng đen "chạy lên trên" khi đổi chế độ: thanh điều khiển trước đây chỉ có chiều cao tối thiểu nên giữ chiều cao cũ từ khung trước khi ảnh xem trước tạm mất. Giờ chiều cao tính cố định mỗi khung hình (max của phần dư dưới ảnh và ước lượng nội dung, cộng thêm 3 hàng TikTok khi hiện); phần dư dưới ảnh được nhớ lại khi ảnh tạm mất.

## 1811.26.2 — 18/11/2026
- Sửa giao diện nhấp nháy lên xuống: bỏ cách đo chiều cao thanh điều khiển ở khung trước để tính khoảng đệm cho khung sau (hai giá trị đuổi nhau). Thanh điều khiển giờ cao tối thiểu bằng phần dư dưới ảnh xem trước và xếp sát mép dưới (bố cục từ dưới lên), không còn vòng phản hồi.

## 1811.26.1 — 18/11/2026
- Sửa TikTok "Không mở được ...mp4": mở tệp bằng file descriptor (setDataSourceFd) thay vì đường dẫn; cũng áp dụng cho tệp nhạc.
- Xem trước nét hơn: chuyển YUV→RGB ở độ phân giải đầy đủ (trước đây giảm một nửa rồi phóng to), chọn khung xem trước lớn hơn (~1,4 triệu điểm ảnh).
- Video hỗ trợ tỉ lệ 4:3 (chọn trong Cài đặt → Tỉ lệ; 4:3 dùng khung như 1440x1080), nếu máy không có thì dùng 16:9.
- Tỉ lệ 16:9: thanh điều khiển nền rất mờ, chỉ hiện zoom, chế độ, chụp; thông báo tự ẩn sau 4 giây; TikTok có nút "⋯ Tùy chọn" để mở/ẩn hàng nhạc, tốc độ, thời lượng.
- Nút zoom/chế độ/đèn pin có nền tối bo tròn để dễ đọc trên ảnh.

## 1811.26 — 18/11/2026
- Sửa thời lượng video sai (9 giây hiện 49:39:09): mốc thời gian camera là mốc tuyệt đối theo giờ máy; giờ chuẩn hoá về 0 cho cả hình và tiếng (quay video, TikTok).
- Xem trước phủ kín: ảnh xem trước nằm ngay dưới thanh trên, rộng hết màn hình; phần dư phía dưới do thanh điều khiển lấp (không còn dải đen). Tỉ lệ 16:9 thì thanh điều khiển trong suốt đè lên phần dưới của ảnh.
- Live có âm thanh: mic ghi liên tục vào bộ đệm vòng cùng với hình, clip Live ghép cả tiếng (cần quyền micro và bật "Ghi âm thanh").
- TikTok: ghi lỗi chi tiết (hiện ở dòng trạng thái, lưu thêm csan_log.txt trong thư mục app) và mở tệp đầu ra ở chế độ đọc-ghi; chuẩn hoá mốc thời gian khi đổi tốc độ.

## 1711.26.3 — 17/11/2026
- Thêm icon ứng dụng (máy ảnh, đủ các độ phân giải mipmap).
- Ảnh và video lưu vào thư viện máy ở DCIM/Csan (qua MediaStore, không cần thêm quyền); xong thì xoá bản tạm trong thư mục app. Nếu không ghi được thì giữ tệp trong thư mục app và báo lý do.
- Zoom mượt: chụm 2 ngón hoặc bấm nút 1x/2x/3x/5x chỉ đổi mục tiêu, hệ số zoom trượt dần theo từng khung hình.
- Bỏ CameraX khỏi kế hoạch; Camera API 1 để sau.

## 1711.26.2 — 17/11/2026
- Sửa font: đính kèm DejaVu Sans (đủ dấu tiếng Việt), hết ô vuông.
- Sửa xem trước bị xoay ngang: bỏ đọc mã khoá SENSOR_ORIENTATION (đọc sai → 0); dùng 90° (sau) / 270° (trước), thêm mục "Xoay xem trước/ảnh thêm" trong Cài đặt để chỉnh nếu máy khác.
- TikTok: tốc độ và thời lượng chuyển thành nút chọn luôn hiện ở phía dưới; danh sách nhạc báo rõ lỗi JNI/quyền và tự quét thư mục Music/Download… nếu MediaStore trống.
- Nút thanh trên to hơn ~20%.

## 1711.26.1 — 17/11/2026
- Sửa bước đặt tên APK trong workflow (cp lỗi khi có nhiều tệp khớp `*.apk`): chọn đúng `CSan.apk`, in danh sách thư mục để chẩn đoán. Build + ký APK đã thành công.
- Bỏ cảnh báo `pos_ms` không dùng.

## 1711.26 — 17/11/2026
- Sửa lỗi biên dịch: eframe 0.29.1 không có `NativeOptions::android_app`; chuyển sang `event_loop_builder` + `with_android_app` của winit (thêm phụ thuộc `winit` 0.30 với `android-native-activity`).
- Ghi rõ kiểu f32 cho các `Stroke::new` để hết cảnh báo của trình biên dịch mới.

## 1511.26.3 — 15/11/2026
- Sửa lỗi build: cargo-apk 0.10 không cho đặt version_name/version_code trong Cargo.toml. Bỏ hai dòng đó; `version` Cargo đổi sang YY.MM.DD (26.11.15-3) để cargo-apk tự suy versionCode tăng đúng theo ngày; số hiển thị 1511.26.3 nằm ở `display_version` (dùng cho tiêu đề app và tên APK).

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
