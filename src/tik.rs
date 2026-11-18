//! Hậu xử lý TikTok (port TikMux.java): đổi tốc độ video bằng cách co/giãn mốc thời gian (không mã hóa lại hình);
//! nếu có nhạc: cắt đoạn nhạc -> giải mã -> AAC -> ghép (nhạc luôn ở tốc độ thường).
use crate::video::*;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::ptr::null_mut;
use std::time::{Duration, Instant};

struct Aac {
    frames: Vec<(Vec<u8>, BufInfo)>,
    fmt: *mut AMediaFormat,
}

unsafe fn mime_of(f: *const AMediaFormat) -> String {
    let mut p: *const std::ffi::c_char = std::ptr::null();
    if AMediaFormat_getString(f, cs("mime").as_ptr(), &mut p) && !p.is_null() {
        return std::ffi::CStr::from_ptr(p).to_string_lossy().to_string();
    }
    String::new()
}

unsafe fn open_extractor(path: &Path, prefix: &str) -> Result<(*mut AMediaExtractor, usize, *mut AMediaFormat, std::fs::File), String> {
    // mở bằng file descriptor (đường dẫn trong Android/data thường không mở được bằng setDataSource)
    let file = std::fs::File::open(path).map_err(|e| format!("Không mở được {}: {}", path.display(), e))?;
    let len = file.metadata().map(|m| m.len() as i64).unwrap_or(0);
    let ex = AMediaExtractor_new();
    if ex.is_null() {
        return Err("Không tạo được extractor".to_string());
    }
    let st = AMediaExtractor_setDataSourceFd(ex, file.as_raw_fd(), 0, len);
    if st != 0 {
        AMediaExtractor_delete(ex);
        return Err(format!("Extractor không đọc được {} (mã {})", path.display(), st));
    }
    let n = AMediaExtractor_getTrackCount(ex);
    for i in 0..n {
        let f = AMediaExtractor_getTrackFormat(ex, i);
        if !f.is_null() && mime_of(f).starts_with(prefix) {
            AMediaExtractor_selectTrack(ex, i);
            return Ok((ex, i, f, file));
        }
        if !f.is_null() {
            AMediaFormat_delete(f);
        }
    }
    AMediaExtractor_delete(ex);
    Err(format!("Tệp không có track {}", prefix))
}

/// Giải mã đoạn nhạc [start_us, start_us+len_us) -> PCM 16-bit -> AAC.
unsafe fn encode_music(path: &Path, start_us: i64, len_us: i64) -> Result<Aac, String> {
    let (ax, _tr, afmt, _afile) = open_extractor(path, "audio/")?;
    AMediaExtractor_seekTo(ax, start_us, 2);
    let dec = AMediaCodec_createDecoderByType(cs(&mime_of(afmt)).as_ptr());
    if dec.is_null() {
        AMediaExtractor_delete(ax);
        return Err("Không có bộ giải mã cho file nhạc".to_string());
    }
    if AMediaCodec_configure(dec, afmt, null_mut(), null_mut(), 0) != 0 || AMediaCodec_start(dec) != 0 {
        AMediaCodec_delete(dec);
        AMediaExtractor_delete(ax);
        return Err("Giải mã nhạc lỗi".to_string());
    }
    AMediaFormat_delete(afmt);

    let mut enc: *mut AMediaCodec = null_mut();
    let (mut rate, mut ch) = (44100i32, 2i32);
    let mut pcm: Vec<u8> = Vec::new();
    let mut pos = 0usize;
    let mut fed_frames: i64 = 0;
    let mut dec_in_eos = false;
    let mut dec_out_eos = false;
    let mut enc_in_eos = false;
    let mut enc_done = false;
    let mut out = Aac { frames: Vec::new(), fmt: null_mut() };
    let t0 = Instant::now();
    let mut err: Option<String> = None;

    while !enc_done && t0.elapsed() < Duration::from_secs(90) {
        // 1) nạp bộ giải mã
        if !dec_in_eos {
            let idx = AMediaCodec_dequeueInputBuffer(dec, 0);
            if idx >= 0 {
                let mut cap = 0usize;
                let p = AMediaCodec_getInputBuffer(dec, idx as usize, &mut cap);
                let n = if p.is_null() { -1 } else { AMediaExtractor_readSampleData(ax, p, cap) };
                let st = AMediaExtractor_getSampleTime(ax);
                if n < 0 || st - start_us >= len_us + 500_000 {
                    AMediaCodec_queueInputBuffer(dec, idx as usize, 0, 0, 0, FLAG_EOS);
                    dec_in_eos = true;
                } else {
                    AMediaCodec_queueInputBuffer(dec, idx as usize, 0, n as usize, st.max(0) as u64, 0);
                    AMediaExtractor_advance(ax);
                }
            }
        }
        // 2) lấy PCM từ bộ giải mã khi hàng đợi còn ít
        if !dec_out_eos && pcm.len() - pos < 65536 {
            let mut info = BufInfo { offset: 0, size: 0, pts: 0, flags: 0 };
            let o = AMediaCodec_dequeueOutputBuffer(dec, &mut info, 0);
            if o == -2 {
                let f = AMediaCodec_getOutputFormat(dec);
                if !f.is_null() {
                    let (mut r, mut c) = (0i32, 0i32);
                    if AMediaFormat_getInt32(f, cs("sample-rate").as_ptr(), &mut r) && r > 0 {
                        rate = r;
                    }
                    if AMediaFormat_getInt32(f, cs("channel-count").as_ptr(), &mut c) && c > 0 {
                        ch = c;
                    }
                    AMediaFormat_delete(f);
                }
            } else if o >= 0 {
                if enc.is_null() {
                    enc = AMediaCodec_createEncoderByType(cs("audio/mp4a-latm").as_ptr());
                    if enc.is_null() {
                        err = Some("Không có bộ mã hóa AAC".to_string());
                        break;
                    }
                    let f = AMediaFormat_new();
                    AMediaFormat_setString(f, cs("mime").as_ptr(), cs("audio/mp4a-latm").as_ptr());
                    AMediaFormat_setInt32(f, cs("sample-rate").as_ptr(), rate);
                    AMediaFormat_setInt32(f, cs("channel-count").as_ptr(), ch);
                    AMediaFormat_setInt32(f, cs("bitrate").as_ptr(), 128_000 * ch.min(2));
                    AMediaFormat_setInt32(f, cs("aac-profile").as_ptr(), 2);
                    AMediaFormat_setInt32(f, cs("max-input-size").as_ptr(), 65536);
                    let st = AMediaCodec_configure(enc, f, null_mut(), null_mut(), 1);
                    AMediaFormat_delete(f);
                    if st != 0 || AMediaCodec_start(enc) != 0 {
                        err = Some("Cấu hình AAC lỗi".to_string());
                        break;
                    }
                }
                let mut sz = 0usize;
                let p = AMediaCodec_getOutputBuffer(dec, o as usize, &mut sz);
                if !p.is_null() && info.size > 0 {
                    if pos > 0 && pos == pcm.len() {
                        pcm.clear();
                        pos = 0;
                    }
                    pcm.extend_from_slice(std::slice::from_raw_parts(p.add(info.offset as usize), info.size as usize));
                }
                if (info.flags & FLAG_EOS) != 0 {
                    dec_out_eos = true;
                }
                AMediaCodec_releaseOutputBuffer(dec, o as usize, false);
            }
        }
        // 3) nạp bộ mã hóa AAC
        if !enc.is_null() && !enc_in_eos {
            let idx = AMediaCodec_dequeueInputBuffer(enc, 0);
            if idx >= 0 {
                let frame_bytes = (ch.max(1) * 2) as usize;
                let have = pcm.len() - pos;
                let reached = fed_frames * 1_000_000 / rate as i64 >= len_us;
                if reached || (dec_out_eos && have == 0) {
                    AMediaCodec_queueInputBuffer(enc, idx as usize, 0, 0, (fed_frames * 1_000_000 / rate as i64) as u64, FLAG_EOS);
                    enc_in_eos = true;
                } else if have >= frame_bytes {
                    let mut cap = 0usize;
                    let p = AMediaCodec_getInputBuffer(enc, idx as usize, &mut cap);
                    let mut take = have.min(cap.max(frame_bytes)).min(8192 * frame_bytes / 2);
                    take -= take % frame_bytes;
                    if p.is_null() || take == 0 || take > cap {
                        AMediaCodec_queueInputBuffer(enc, idx as usize, 0, 0, 0, 0);
                    } else {
                        std::ptr::copy_nonoverlapping(pcm.as_ptr().add(pos), p, take);
                        let pts = fed_frames * 1_000_000 / rate as i64;
                        AMediaCodec_queueInputBuffer(enc, idx as usize, 0, take, pts as u64, 0);
                        pos += take;
                        fed_frames += (take / frame_bytes) as i64;
                    }
                } else {
                    AMediaCodec_queueInputBuffer(enc, idx as usize, 0, 0, 0, 0);
                }
            }
        }
        // 4) lấy AAC
        if !enc.is_null() {
            let mut info = BufInfo { offset: 0, size: 0, pts: 0, flags: 0 };
            let o = AMediaCodec_dequeueOutputBuffer(enc, &mut info, 0);
            if o == -2 {
                if !out.fmt.is_null() {
                    AMediaFormat_delete(out.fmt);
                }
                out.fmt = AMediaCodec_getOutputFormat(enc);
            } else if o >= 0 {
                let mut sz = 0usize;
                let p = AMediaCodec_getOutputBuffer(enc, o as usize, &mut sz);
                if !p.is_null() && info.size > 0 && (info.flags & FLAG_CONFIG) == 0 {
                    let d = std::slice::from_raw_parts(p.add(info.offset as usize), info.size as usize).to_vec();
                    let bi = BufInfo { offset: 0, size: d.len() as i32, pts: info.pts, flags: 0 };
                    out.frames.push((d, bi));
                }
                if (info.flags & FLAG_EOS) != 0 {
                    enc_done = true;
                }
                AMediaCodec_releaseOutputBuffer(enc, o as usize, false);
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    AMediaCodec_stop(dec);
    AMediaCodec_delete(dec);
    if !enc.is_null() {
        AMediaCodec_stop(enc);
        AMediaCodec_delete(enc);
    }
    AMediaExtractor_delete(ax);
    if let Some(e) = err {
        return Err(e);
    }
    if out.fmt.is_null() || out.frames.is_empty() {
        return Err("Không mã hóa được đoạn nhạc".to_string());
    }
    Ok(out)
}

/// speed > 1 nhanh (3x, 2x), < 1 chậm. orient: góc xoay video (độ).
pub fn run(src: &Path, dst: &Path, speed: f32, music: Option<&Path>, music_start_us: i64, seg_us: i64, orient: i32) -> Result<(), String> {
    unsafe {
        let (vx, _vt, vf, _vfile) = open_extractor(src, "video/")?;
        let mut dur_us: i64 = 0;
        AMediaFormat_getInt64(vf, cs("durationUs").as_ptr(), &mut dur_us);
        let final_us = (dur_us as f64 / speed as f64) as i64;

        let mut aac: Option<Aac> = None;
        if let Some(m) = music {
            let len = final_us.min(seg_us);
            if len > 200_000 {
                aac = Some(encode_music(m, music_start_us, len)?);
            }
        }

        let file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(true).open(dst).map_err(|e| e.to_string())?;
        let mux = AMediaMuxer_new(file.as_raw_fd(), 0);
        if mux.is_null() {
            return Err("Không tạo được muxer".to_string());
        }
        AMediaMuxer_setOrientationHint(mux, orient);
        let vi = AMediaMuxer_addTrack(mux, vf);
        let ai = match &aac {
            Some(a) => AMediaMuxer_addTrack(mux, a.fmt),
            None => -1,
        };
        if vi < 0 {
            AMediaMuxer_delete(mux);
            return Err(format!("Muxer không nhận track video (mã {})", vi));
        }
        let st = AMediaMuxer_start(mux);
        if st != 0 {
            AMediaMuxer_delete(mux);
            return Err(format!("Muxer không chạy được (mã {}, track âm thanh {})", st, ai));
        }
        let mut cap = 8usize << 20;
        let mut mx = 0i32;
        if AMediaFormat_getInt32(vf, cs("max-input-size").as_ptr(), &mut mx) && mx as usize > cap {
            cap = mx as usize;
        }
        let mut buf = vec![0u8; cap];
        let mut last: i64 = -1;
        let mut vbase: Option<i64> = None;
        loop {
            let n = AMediaExtractor_readSampleData(vx, buf.as_mut_ptr(), cap);
            if n < 0 {
                break;
            }
            let st_us = AMediaExtractor_getSampleTime(vx);
            if vbase.is_none() {
                vbase = Some(st_us);
            }
            let mut pts = ((st_us - vbase.unwrap_or(0)).max(0) as f64 / speed as f64) as i64;
            if pts <= last {
                pts = last + 1;
            }
            last = pts;
            let fl = if (AMediaExtractor_getSampleFlags(vx) & 1) != 0 { 1 } else { 0 };
            let info = BufInfo { offset: 0, size: n as i32, pts, flags: fl };
            AMediaMuxer_writeSampleData(mux, vi as usize, buf.as_ptr(), &info);
            AMediaExtractor_advance(vx);
        }
        if let Some(a) = &aac {
            if ai >= 0 {
                for (d, bi) in &a.frames {
                    AMediaMuxer_writeSampleData(mux, ai as usize, d.as_ptr(), bi);
                }
            }
            AMediaFormat_delete(a.fmt);
        }
        AMediaMuxer_stop(mux);
        AMediaMuxer_delete(mux);
        AMediaFormat_delete(vf);
        AMediaExtractor_delete(vx);
        drop(file);
        Ok(())
    }
}

/// Thời lượng (ms) của tệp nhạc; 0 nếu không đọc được.
pub fn probe_duration_ms(path: &Path) -> i64 {
    unsafe {
        match open_extractor(path, "audio/") {
            Ok((ex, _t, f, _file)) => {
                let mut d: i64 = 0;
                AMediaFormat_getInt64(f, cs("durationUs").as_ptr(), &mut d);
                AMediaFormat_delete(f);
                AMediaExtractor_delete(ex);
                d / 1000
            }
            Err(_) => 0,
        }
    }
}
