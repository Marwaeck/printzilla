//! Where a file came from: the web page it was downloaded from.
//! macOS keeps that in the "kMDItemWhereFroms" attribute, Windows in the
//! "Zone.Identifier" stream next to the file.

use std::path::Path;

/// The page (or download address) a file came from, if the system remembers it.
pub fn download_url(path: &Path) -> Option<String> {
    let urls = where_froms(path);
    pick(&urls)
}

/// Prefers the page the download started from over the raw download address.
fn pick(urls: &[String]) -> Option<String> {
    let web: Vec<&String> = urls.iter().filter(|u| u.starts_with("http://") || u.starts_with("https://")).collect();
    // a page on a known model site is best
    if let Some(u) = web.iter().find(|u| site_name(u).is_some() && !is_cdn(u)) {
        return Some((*u).clone());
    }
    // otherwise the referrer (second entry), then the download itself
    web.get(1).or(web.first()).map(|u| (*u).clone())
}

fn is_cdn(u: &str) -> bool {
    let h = host(u);
    h.starts_with("cdn") || h.contains(".cdn") || h.starts_with("files.") || h.starts_with("media.") || h.starts_with("public-cdn")
}

pub fn host(u: &str) -> String {
    let rest = u.split("://").nth(1).unwrap_or(u);
    let h = rest.split(['/', '?', '#']).next().unwrap_or("");
    let h = h.rsplit('@').next().unwrap_or(h);
    h.split(':').next().unwrap_or(h).to_lowercase()
}

/// A friendly name for well-known model sites.
pub fn site_name(u: &str) -> Option<&'static str> {
    let h = host(u);
    let h = h.trim_start_matches("www.");
    for (dom, name) in [
        ("makerworld.com", "MakerWorld"),
        ("makerworld.com.cn", "MakerWorld"),
        ("printables.com", "Printables"),
        ("thingiverse.com", "Thingiverse"),
        ("thangs.com", "Thangs"),
        ("cults3d.com", "Cults3D"),
        ("myminifactory.com", "MyMiniFactory"),
        ("grabcad.com", "GrabCAD"),
        ("youmagine.com", "YouMagine"),
        ("github.com", "GitHub"),
        ("githubusercontent.com", "GitHub"),
        ("onshape.com", "Onshape"),
        ("autodesk.com", "Autodesk"),
        ("instructables.com", "Instructables"),
    ] {
        if h == dom || h.ends_with(&format!(".{dom}")) {
            return Some(name);
        }
    }
    None
}

/// Short label for a link: the site name or the host.
pub fn label(u: &str) -> String {
    site_name(u).map(|s| s.to_string()).unwrap_or_else(|| host(u).trim_start_matches("www.").to_string())
}

/// Opens a web address in the browser.
pub fn open_url(u: &str) {
    if !(u.starts_with("http://") || u.starts_with("https://")) {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(u).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // rundll32 opens the address with the default browser without a console window
        let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", u]).creation_flags(0x0800_0000).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(u).spawn();
    }
}

/// Makes a pasted address usable ("printables.com/model/1" → "https://printables.com/model/1").
pub fn normalize(u: &str) -> String {
    let u = u.trim();
    if u.is_empty() || u.contains("://") {
        u.to_string()
    } else if u.contains('.') && !u.contains(' ') {
        format!("https://{u}")
    } else {
        u.to_string()
    }
}

#[cfg(target_os = "macos")]
fn where_froms(path: &Path) -> Vec<String> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let Ok(p) = CString::new(path.as_os_str().as_bytes()) else { return Vec::new() };
    let name = CString::new("com.apple.metadata:kMDItemWhereFroms").unwrap();
    unsafe {
        let n = libc::getxattr(p.as_ptr(), name.as_ptr(), std::ptr::null_mut(), 0, 0, 0);
        if n <= 0 {
            return Vec::new();
        }
        let mut buf = vec![0u8; n as usize];
        let n = libc::getxattr(p.as_ptr(), name.as_ptr(), buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0, 0);
        if n <= 0 {
            return Vec::new();
        }
        buf.truncate(n as usize);
        bplist_strings(&buf)
    }
}

#[cfg(target_os = "windows")]
fn where_froms(path: &Path) -> Vec<String> {
    let mut s = path.as_os_str().to_os_string();
    s.push(":Zone.Identifier");
    let Ok(text) = std::fs::read(&s) else { return Vec::new() };
    zone_identifier_urls(&String::from_utf8_lossy(&text))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn where_froms(_path: &Path) -> Vec<String> {
    Vec::new()
}

/// "HostUrl=…" (the download) and "ReferrerUrl=…" (the page), in that order.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn zone_identifier_urls(text: &str) -> Vec<String> {
    let mut host = None;
    let mut referrer = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("HostUrl=") {
            host = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("ReferrerUrl=") {
            referrer = Some(v.to_string());
        }
    }
    host.into_iter().chain(referrer).filter(|u| u.starts_with("http")).collect()
}

/// All strings in a binary property list (enough for the WhereFroms array).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn bplist_strings(d: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    if d.len() < 40 || &d[0..6] != b"bplist" {
        return out;
    }
    let t = &d[d.len() - 32..];
    let off_size = t[6] as usize;
    let num = u64::from_be_bytes(t[8..16].try_into().unwrap()) as usize;
    let table = u64::from_be_bytes(t[24..32].try_into().unwrap()) as usize;
    let be = |at: usize, n: usize| -> Option<usize> {
        let b = d.get(at..at + n)?;
        Some(b.iter().fold(0usize, |a, &x| (a << 8) | x as usize))
    };
    for i in 0..num.min(1000) {
        let Some(off) = be(table + i * off_size, off_size) else { break };
        let Some(&marker) = d.get(off) else { continue };
        let kind = marker >> 4;
        if kind != 0x5 && kind != 0x6 {
            continue;
        }
        let mut len = (marker & 0x0f) as usize;
        let mut start = off + 1;
        if len == 0x0f {
            let Some(&im) = d.get(off + 1) else { continue };
            let n = 1usize << (im & 0x0f);
            let Some(l) = be(off + 2, n) else { continue };
            len = l;
            start = off + 2 + n;
        }
        if kind == 0x5 {
            if let Some(b) = d.get(start..start + len) {
                out.push(String::from_utf8_lossy(b).into_owned());
            }
        } else if let Some(b) = d.get(start..start + len * 2) {
            let u: Vec<u16> = b.chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
            out.push(String::from_utf16_lossy(&u));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_model_page() {
        let urls = vec![
            "https://public-cdn.makerworld.com/x/y.3mf".to_string(),
            "https://makerworld.com/de/models/12345".to_string(),
        ];
        assert_eq!(pick(&urls).as_deref(), Some("https://makerworld.com/de/models/12345"));
        assert_eq!(label("https://www.printables.com/model/1"), "Printables");
        assert_eq!(label("https://example.org/a"), "example.org");
        assert_eq!(normalize("printables.com/model/1"), "https://printables.com/model/1");
    }

    #[test]
    fn zone_identifier() {
        let t = "[ZoneTransfer]\r\nZoneId=3\r\nReferrerUrl=https://www.thingiverse.com/thing:1\r\nHostUrl=https://cdn.thingiverse.com/a.stl\r\n";
        let u = zone_identifier_urls(t);
        assert_eq!(pick(&u).as_deref(), Some("https://www.thingiverse.com/thing:1"));
    }

    #[test]
    fn binary_plist() {
        // bplist00 with ["https://a.example/x.stl", "https://www.printables.com/model/7"]
        let a = b"https://a.example/x.stl";
        let b = b"https://www.printables.com/model/7";
        let mut d = b"bplist00".to_vec();
        let o0 = d.len();
        d.extend([0xA2, 1, 2]);
        let o1 = d.len();
        d.push(0x5F);
        d.extend([0x10, a.len() as u8]);
        d.extend(a);
        let o2 = d.len();
        d.push(0x5F);
        d.extend([0x10, b.len() as u8]);
        d.extend(b);
        let table = d.len();
        d.extend([o0 as u8, o1 as u8, o2 as u8]);
        let mut trailer = vec![0u8; 6];
        trailer.push(1); // offset size
        trailer.push(1); // ref size
        trailer.extend(3u64.to_be_bytes());
        trailer.extend(0u64.to_be_bytes());
        trailer.extend((table as u64).to_be_bytes());
        d.extend(trailer);
        let s = bplist_strings(&d);
        assert_eq!(s, vec![String::from_utf8_lossy(a), String::from_utf8_lossy(b)]);
        assert_eq!(pick(&s).as_deref(), Some("https://www.printables.com/model/7"));
    }
}
