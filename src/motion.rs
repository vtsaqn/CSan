//! Ảnh động (Motion Photo / Live): JPEG + XMP (Google + OPlus) + MP4 nối đuôi. Port từ HeifXmp/MainActivity.
const XMP_NS: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const EXIF_HDR: &[u8] = b"Exif\0\0";

fn xmp(vs: usize, img_mime: &str, pre_us: i64) -> Vec<u8> {
    let x = format!(
        "<?xpacket begin=\"\u{FEFF}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"Adobe XMP Core Test.SNAPSHOT\">\n\
 <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
  <rdf:Description rdf:about=\"\"\n\
   xmlns:GCamera=\"http://ns.google.com/photos/1.0/camera/\"\n\
   GCamera:MotionPhoto=\"1\"\n\
   GCamera:MotionPhotoVersion=\"1\"\n\
   GCamera:MotionPhotoPresentationTimestampUs=\"{pre}\"\n\
   GCamera:MotionPhotoPrimaryPresentationTimestampUs=\"{pre}\"\n\
   GCamera:MotionPhotoOwner=\"oplus\"/>\n\
  <rdf:Description rdf:about=\"\"\n\
   xmlns:OLP=\"http://ns.oplus.com/photo/1.0/livephoto/\"\n\
   OLP:OLivePhotoVersion=\"2\"\n\
   OLP:VideoLength=\"{vs}\"\n\
   OLP:MotionPhotoFeatureFlag=\"1\"/>\n\
  <rdf:Description rdf:about=\"\"\n\
   xmlns:Container=\"http://ns.google.com/photos/1.0/container/\"\n\
   xmlns:Item=\"http://ns.google.com/photos/1.0/container/item/\">\n\
   <Container:Directory>\n\
    <rdf:Seq>\n\
     <rdf:li rdf:parseType=\"Resource\">\n\
      <Container:Item Item:Mime=\"{mime}\" Item:Semantic=\"Primary\" Item:Length=\"0\" Item:Padding=\"0\"/>\n\
     </rdf:li>\n\
     <rdf:li rdf:parseType=\"Resource\">\n\
      <Container:Item Item:Mime=\"video/mp4\" Item:Semantic=\"MotionPhoto\" Item:Length=\"{vs}\" Item:Padding=\"0\"/>\n\
     </rdf:li>\n\
    </rdf:Seq>\n\
   </Container:Directory>\n\
  </rdf:Description>\n\
 </rdf:RDF>\n\
</x:xmpmeta>\n\
<?xpacket end=\"w\"?>",
        vs = vs,
        mime = img_mime,
        pre = pre_us
    );
    x.into_bytes()
}

fn u16be(b: &[u8], i: usize) -> usize {
    ((b[i] as usize) << 8) | b[i + 1] as usize
}

fn starts(b: &[u8], off: usize, p: &[u8]) -> bool {
    off + p.len() <= b.len() && &b[off..off + p.len()] == p
}

fn app1(xmp: &[u8]) -> Vec<u8> {
    let plen = XMP_NS.len() + xmp.len();
    let mut o = vec![0xff, 0xe1, (((plen + 2) >> 8) & 0xff) as u8, ((plen + 2) & 0xff) as u8];
    o.extend_from_slice(XMP_NS);
    o.extend_from_slice(xmp);
    o
}

fn strip_old_xmp(j: &[u8]) -> Result<Vec<u8>, String> {
    if j.len() < 2 || j[0] != 0xff || j[1] != 0xd8 {
        return Err("File không phải JPEG hợp lệ".to_string());
    }
    let mut out = vec![0xff, 0xd8];
    let mut i = 2usize;
    while i + 1 < j.len() {
        if j[i] != 0xff {
            out.extend_from_slice(&j[i..]);
            break;
        }
        let m = j[i + 1];
        if m == 0xd9 || m == 0xda || i + 4 > j.len() {
            out.extend_from_slice(&j[i..]);
            break;
        }
        let sl = u16be(j, i + 2);
        if m == 0xe1 && starts(j, i + 4, XMP_NS) {
            i += 2 + sl;
            continue;
        }
        let end = (i + 2 + sl).min(j.len());
        out.extend_from_slice(&j[i..end]);
        i += 2 + sl;
    }
    Ok(out)
}

fn insert_xmp(j: &[u8], a1: &[u8]) -> Vec<u8> {
    let mut r = vec![0xff, 0xd8];
    let mut p = 2usize;
    while p + 1 < j.len() {
        if j[p] != 0xff {
            break;
        }
        let m = j[p + 1];
        if m == 0xd9 || m == 0xda || p + 4 > j.len() {
            break;
        }
        let sl = u16be(j, p + 2);
        let jfif = m == 0xe0;
        let exif = m == 0xe1 && starts(j, p + 4, EXIF_HDR);
        if jfif || exif {
            r.extend_from_slice(&j[p..(p + 2 + sl).min(j.len())]);
            p += 2 + sl;
        } else {
            break;
        }
    }
    r.extend_from_slice(a1);
    if p < j.len() {
        r.extend_from_slice(&j[p..]);
    }
    r
}

/// JPEG + MP4 -> Motion Photo (một tệp .jpg).
pub fn build_motion_photo(jpeg: &[u8], video: &[u8], pre_us: i64) -> Result<Vec<u8>, String> {
    let clean = strip_old_xmp(jpeg)?;
    let a = app1(&xmp(video.len(), "image/jpeg", pre_us));
    let mut fin = insert_xmp(&clean, &a);
    fin.extend_from_slice(video);
    Ok(fin)
}
