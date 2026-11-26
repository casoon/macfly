use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use macfly_core::config::ProfileConfig;
use macfly_core::dlp::apply_dlp;
use macfly_core::error::MacflyError;
use macfly_core::model::{
    PhotoAlbum, PhotoItem, PhotosAlbumsResult, PhotosListResult, sha256_file_hex,
};
use macfly_core::plan::{ImageFormat, PhotoConvert, PlanOperation, SidecarFormat};
use macfly_core::plan_text::Lang;
use serde::Deserialize;

use crate::jxa::run_jxa;

const SCRIPT_ALBUMS: &str = include_str!("../scripts/photos_albums.js");
const SCRIPT_LIST: &str = include_str!("../scripts/photos_list.js");
const SCRIPT_EXPORT_ITEM: &str = include_str!("../scripts/photos_export_item.js");

/// Originale aus iCloud müssen erst geladen werden; ein einzelnes Foto darf deshalb länger dauern.
const EXPORT_MIN_TIMEOUT_SECS: u64 = 180;

/// Dateiendungen, die `sips` als Bild lesen kann. Videos (mov, mp4, …) werden nie umgewandelt.
const CONVERTIBLE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "heic", "heif", "tif", "tiff", "gif", "bmp", "dng", "cr2", "cr3", "nef",
    "arw", "raf", "orf", "rw2",
];

pub struct PhotosAdapter {
    timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
struct RawAlbums {
    albums: Vec<PhotoAlbum>,
}

#[derive(Debug, Deserialize)]
struct RawList {
    total_matched: usize,
    items: Vec<PhotoItem>,
}

#[derive(Debug, Deserialize)]
struct RawExportItem {
    meta: PhotoItem,
}

/// Ergebnis eines einzelnen exportierten Fotos.
#[derive(Debug, Default)]
struct ItemOutcome {
    /// (Pfad, SHA-256) der Dateien, die neu geschrieben wurden.
    written: Vec<(PathBuf, String)>,
    /// Dateien, die schon identisch im Ziel lagen.
    skipped: usize,
    converted: usize,
    /// Dateien, die nicht umgewandelt werden konnten (z. B. Videos).
    not_converted: usize,
}

impl PhotosAdapter {
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    pub fn albums(&self) -> Result<PhotosAlbumsResult, MacflyError> {
        let raw: RawAlbums = run_jxa(
            "Apple Photos",
            SCRIPT_ALBUMS,
            &serde_json::json!({}),
            self.timeout_secs,
        )?;
        Ok(PhotosAlbumsResult {
            schema_version: "1.0".to_string(),
            total_albums: raw.albums.len(),
            albums: raw.albums,
        })
    }

    /// Titel und Beschreibungen laufen durch die DLP-Firewall des Profils.
    pub fn list(
        &self,
        profile: &ProfileConfig,
        album: Option<&str>,
        keyword: Option<&str>,
        since: Option<&str>,
        limit: usize,
    ) -> Result<PhotosListResult, MacflyError> {
        let payload = serde_json::json!({
            "album": album,
            "keyword": keyword,
            "since": since,
            "limit": limit,
        });
        let raw: RawList = run_jxa("Apple Photos", SCRIPT_LIST, &payload, self.timeout_secs)?;
        let mut items = raw.items;
        for item in &mut items {
            for field in [&mut item.title, &mut item.description] {
                if let Some(text) = field.take() {
                    *field = Some(apply_dlp(&text, &profile.dlp).text);
                }
            }
        }
        let returned = items.len();
        Ok(PhotosListResult {
            schema_version: "1.0".to_string(),
            total_matched: raw.total_matched,
            returned,
            items,
        })
    }

    pub fn execute_operation(
        &self,
        profile: &ProfileConfig,
        op: &PlanOperation,
    ) -> Result<String, MacflyError> {
        let PlanOperation::PhotosExport {
            media_ids,
            destination,
            originals,
            sidecar,
            convert,
            ..
        } = op
        else {
            return Err(MacflyError::PlanError(
                "Photos-Adapter kann diese Operation nicht ausführen".to_string(),
            ));
        };

        // Die Profil-Regeln können sich seit der Planerstellung geändert haben.
        let dest = profile.export.check_destination(destination)?;
        fs::create_dir_all(&dest)?;

        let timeout = self.timeout_secs.max(EXPORT_MIN_TIMEOUT_SECS);
        let run_id = macfly_core::plan::now_unix();
        let mut written: Vec<(PathBuf, String)> = Vec::new();
        let mut skipped = 0usize;
        let mut items_new = 0usize;
        let mut items_present = 0usize;
        let mut converted = 0usize;
        let mut not_converted = 0usize;
        let mut failures: Vec<String> = Vec::new();

        for (idx, id) in media_ids.iter().enumerate() {
            let staging = dest.join(format!(".macfly-staging-{run_id}-{idx}"));
            fs::create_dir_all(&staging)?;
            let result =
                self.export_one(id, &staging, &dest, *originals, *sidecar, *convert, timeout);
            let _ = fs::remove_dir_all(&staging);
            match result {
                Ok(o) => {
                    if o.written.is_empty() {
                        items_present += 1;
                    } else {
                        items_new += 1;
                    }
                    written.extend(o.written);
                    skipped += o.skipped;
                    converted += o.converted;
                    not_converted += o.not_converted;
                }
                // Fehlende Automatisierungsrechte betreffen alle weiteren Fotos genauso.
                Err(e @ MacflyError::TccPermissionDenied { .. }) => return Err(e),
                Err(e) => failures.push(format!("{id}: {e}")),
            }
        }

        let manifest = write_manifest(&dest, run_id, &written)?;
        let mut msg = format!(
            "{} Datei(en) nach '{}' geschrieben, {} bereits vorhanden",
            written.len(),
            dest.display(),
            skipped
        );
        if convert.is_some() {
            msg.push_str(&format!(", {converted} umgewandelt"));
            if not_converted > 0 {
                msg.push_str(&format!(
                    " ({not_converted} nicht umwandelbar, unverändert übernommen)"
                ));
            }
        }
        if let Some((path, hash)) = &manifest {
            msg.push_str(&format!("; Prüfsummen: {} (sha256:{hash})", path.display()));
        }
        if failures.is_empty() {
            // Erste Zeile für Menschen, danach die technischen Angaben (Journal, `--details`).
            let friendly =
                friendly_summary(Lang::detect(), items_new, items_present, converted, &dest);
            Ok(format!("{friendly}\n{msg}"))
        } else {
            Err(MacflyError::ScriptError(format!(
                "{msg}; {} von {} Foto(s) fehlgeschlagen:\n{}",
                failures.len(),
                media_ids.len(),
                failures.join("\n")
            )))
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn export_one(
        &self,
        id: &str,
        staging: &Path,
        dest: &Path,
        originals: bool,
        sidecar: SidecarFormat,
        convert: Option<PhotoConvert>,
        timeout: u64,
    ) -> Result<ItemOutcome, MacflyError> {
        let payload = serde_json::json!({
            "id": id,
            "staging_dir": staging.display().to_string(),
            "originals": originals,
        });
        let raw: RawExportItem = run_jxa("Apple Photos", SCRIPT_EXPORT_ITEM, &payload, timeout)?;

        let mut files = visible_files(staging)?;
        if files.is_empty() {
            return Err(MacflyError::ScriptError(
                "Apple Photos hat keine Datei geliefert (Original noch nicht aus iCloud geladen oder nicht verfügbar?)"
                    .to_string(),
            ));
        }
        files.sort();

        let mut outcome = ItemOutcome::default();
        let mut finals: Vec<PathBuf> = Vec::new();
        for file in files {
            let mut current = file;
            if let Some(conv) = convert
                && is_convertible(&current)
            {
                if let Some(new_path) = convert_image(&current, conv)? {
                    if new_path != current {
                        let _ = fs::remove_file(&current);
                    }
                    current = new_path;
                    outcome.converted += 1;
                }
            } else if convert.is_some() {
                outcome.not_converted += 1;
            }
            let name = current
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let (path, was_new) = place_file(&current, dest, &name)?;
            if was_new {
                let hash = sha256_file_hex(&path)?;
                outcome.written.push((path.clone(), hash));
            } else {
                outcome.skipped += 1;
            }
            finals.push(path);
        }

        // Begleitdateien heißen wie die Hauptdatei (bei Live Photos das Standbild, nicht das Video).
        if sidecar != SidecarFormat::None
            && let Some(primary) = pick_primary(&finals)
        {
            if sidecar.xmp() {
                let content = xmp_for(&raw.meta);
                write_sidecar(&primary, "xmp", &content, &mut outcome)?;
            }
            if sidecar.json() {
                let content = serde_json::to_string_pretty(&raw.meta)? + "\n";
                write_sidecar(&primary, "json", &content, &mut outcome)?;
            }
        }
        Ok(outcome)
    }
}

fn friendly_summary(
    lang: Lang,
    new: usize,
    present: usize,
    converted: usize,
    dest: &Path,
) -> String {
    let mut line = match lang {
        Lang::De if new == 1 => "1 Foto exportiert".to_string(),
        Lang::De => format!("{new} Fotos exportiert"),
        Lang::En if new == 1 => "1 photo exported".to_string(),
        Lang::En => format!("{new} photos exported"),
    };
    if present > 0 {
        line.push_str(&match lang {
            Lang::De if present == 1 => ", 1 war schon vorhanden".to_string(),
            Lang::De => format!(", {present} waren schon vorhanden"),
            Lang::En => format!(", {present} already there"),
        });
    }
    if converted > 0 {
        line.push_str(&match lang {
            Lang::De => format!(", {converted} umgewandelt"),
            Lang::En => format!(", {converted} converted"),
        });
    }
    line.push_str(&format!(" → {}", dest.display()));
    line
}

fn visible_files(dir: &Path) -> Result<Vec<PathBuf>, MacflyError> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || !entry.path().is_file() {
            continue;
        }
        out.push(entry.path());
    }
    Ok(out)
}

fn extension_lower(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn is_convertible(path: &Path) -> bool {
    CONVERTIBLE_EXTENSIONS.contains(&extension_lower(path).as_str())
}

fn is_video(path: &Path) -> bool {
    matches!(extension_lower(path).as_str(), "mov" | "mp4" | "m4v")
}

fn pick_primary(finals: &[PathBuf]) -> Option<PathBuf> {
    finals
        .iter()
        .find(|p| !is_video(p))
        .or_else(|| finals.first())
        .cloned()
}

/// Verschiebt `src` nach `dest/name`. Überschreibt nie: Ist dort schon eine identische Datei,
/// zählt das als bereits exportiert (`false`); eine andere Datei gleichen Namens bekommt ein Suffix.
fn place_file(src: &Path, dest: &Path, name: &str) -> Result<(PathBuf, bool), MacflyError> {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    let src_len = fs::metadata(src)?.len();
    let mut n = 0u32;
    loop {
        let candidate = if n == 0 {
            dest.join(name)
        } else {
            dest.join(format!("{stem}-{n}{ext}"))
        };
        if !candidate.exists() {
            fs::rename(src, &candidate)?;
            return Ok((candidate, true));
        }
        if fs::metadata(&candidate)?.len() == src_len
            && sha256_file_hex(&candidate)? == sha256_file_hex(src)?
        {
            fs::remove_file(src)?;
            return Ok((candidate, false));
        }
        n += 1;
    }
}

fn write_sidecar(
    primary: &Path,
    ext: &str,
    content: &str,
    outcome: &mut ItemOutcome,
) -> Result<(), MacflyError> {
    let target = primary.with_extension(ext);
    if target.exists() && fs::read_to_string(&target).ok().as_deref() == Some(content) {
        outcome.skipped += 1;
        return Ok(());
    }
    let dir = primary.parent().unwrap_or(Path::new("."));
    let tmp = dir.join(format!(".macfly-sidecar-{}.tmp", std::process::id()));
    fs::write(&tmp, content)?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let (path, was_new) = place_file(&tmp, dir, &name)?;
    if was_new {
        let hash = sha256_file_hex(&path)?;
        outcome.written.push((path, hash));
    } else {
        outcome.skipped += 1;
    }
    Ok(())
}

/// Schreibt `macfly-export-<run>.sha256` im Format von `shasum -a 256 -c`.
fn write_manifest(
    dest: &Path,
    run_id: u64,
    written: &[(PathBuf, String)],
) -> Result<Option<(PathBuf, String)>, MacflyError> {
    if written.is_empty() {
        return Ok(None);
    }
    let mut body = String::new();
    for (path, hash) in written {
        let rel = path.strip_prefix(dest).unwrap_or(path);
        body.push_str(&format!("{hash}  {}\n", rel.display()));
    }
    let path = dest.join(format!("macfly-export-{run_id}.sha256"));
    fs::write(&path, &body)?;
    let hash = sha256_file_hex(&path)?;
    Ok(Some((path, hash)))
}

/// Wandelt eine exportierte Kopie mit `sips` um. `None`, wenn nichts zu tun war.
fn convert_image(src: &Path, conv: PhotoConvert) -> Result<Option<PathBuf>, MacflyError> {
    let target_ext = conv.format.extension();
    let same_format = matches!(
        (extension_lower(src).as_str(), conv.format),
        ("jpg" | "jpeg", ImageFormat::Jpeg)
            | ("png", ImageFormat::Png)
            | ("tif" | "tiff", ImageFormat::Tiff)
            | ("heic", ImageFormat::Heic)
    );

    // `sips -Z` würde kleine Bilder auch vergrößern, deshalb nur bei größeren Bildern setzen.
    let resize = match conv.max_size {
        Some(max) => {
            let (w, h) = sips_dimensions(src)?;
            (w.max(h) > max).then_some(max)
        }
        None => None,
    };
    if same_format && resize.is_none() && conv.quality.is_none() {
        return Ok(None);
    }

    let out = src.with_extension(format!("converted.{target_ext}"));
    let mut cmd = Command::new("sips");
    cmd.arg("-s").arg("format").arg(conv.format.sips_name());
    if let Some(q) = conv.quality
        && matches!(conv.format, ImageFormat::Jpeg | ImageFormat::Heic)
    {
        cmd.arg("-s").arg("formatOptions").arg(q.to_string());
    }
    if let Some(max) = resize {
        cmd.arg("-Z").arg(max.to_string());
    }
    cmd.arg(src).arg("--out").arg(&out);
    let output = cmd.output().map_err(|e| {
        MacflyError::ScriptError(format!("sips konnte nicht gestartet werden: {e}"))
    })?;
    if !output.status.success() || !out.exists() {
        return Err(MacflyError::ScriptError(format!(
            "sips-Umwandlung von '{}' fehlgeschlagen: {}",
            src.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    // Endgültiger Name: Originalname mit neuer Endung.
    let final_path = src.with_extension(target_ext);
    if final_path != src {
        fs::rename(&out, &final_path)?;
        Ok(Some(final_path))
    } else {
        // Gleiche Endung (z. B. JPEG verkleinert): die Quelle wird ersetzt.
        fs::rename(&out, src)?;
        Ok(Some(src.to_path_buf()))
    }
}

fn sips_dimensions(path: &Path) -> Result<(u32, u32), MacflyError> {
    let output = Command::new("sips")
        .args(["-g", "pixelWidth", "-g", "pixelHeight"])
        .arg(path)
        .output()
        .map_err(|e| {
            MacflyError::ScriptError(format!("sips konnte nicht gestartet werden: {e}"))
        })?;
    let text = String::from_utf8_lossy(&output.stdout);
    let read = |key: &str| {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(key))
            .and_then(|v| v.trim_start_matches(':').trim().parse::<u32>().ok())
    };
    match (read("pixelWidth"), read("pixelHeight")) {
        (Some(w), Some(h)) => Ok((w, h)),
        _ => Err(MacflyError::ScriptError(format!(
            "Bildgröße von '{}' nicht lesbar",
            path.display()
        ))),
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn gps_coordinate(value: f64, positive: char, negative: char) -> String {
    let abs = value.abs();
    let deg = abs.floor();
    let minutes = (abs - deg) * 60.0;
    let hemisphere = if value >= 0.0 { positive } else { negative };
    format!("{},{:.4}{}", deg as u32, minutes, hemisphere)
}

/// XMP-Begleitdatei mit den Feldern, die Apple Photos per Skript herausgibt.
/// Nutzt Standardfelder (Dublin Core, XMP, Photoshop, EXIF), die Lightroom, digiKam & Co. lesen.
pub fn xmp_for(item: &PhotoItem) -> String {
    let mut attrs = String::new();
    if let Some(date) = &item.date {
        attrs.push_str(&format!(
            "\n    xmp:CreateDate=\"{d}\"\n    photoshop:DateCreated=\"{d}\"",
            d = xml_escape(date)
        ));
    }
    if let (Some(lat), Some(lon)) = (item.latitude, item.longitude) {
        attrs.push_str(&format!(
            "\n    exif:GPSVersionID=\"2.2.0.0\"\n    exif:GPSLatitude=\"{}\"\n    exif:GPSLongitude=\"{}\"",
            gps_coordinate(lat, 'N', 'S'),
            gps_coordinate(lon, 'E', 'W')
        ));
        if let Some(alt) = item.altitude {
            attrs.push_str(&format!(
                "\n    exif:GPSAltitudeRef=\"{}\"\n    exif:GPSAltitude=\"{}/100\"",
                if alt >= 0.0 { 0 } else { 1 },
                (alt.abs() * 100.0).round() as u64
            ));
        }
    }

    let mut body = String::new();
    if let Some(title) = &item.title {
        body.push_str(&format!(
            "   <dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:title>\n",
            xml_escape(title)
        ));
    }
    if let Some(desc) = &item.description {
        body.push_str(&format!(
            "   <dc:description><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:description>\n",
            xml_escape(desc)
        ));
    }
    if !item.keywords.is_empty() {
        body.push_str("   <dc:subject><rdf:Bag>\n");
        for k in &item.keywords {
            body.push_str(&format!("    <rdf:li>{}</rdf:li>\n", xml_escape(k)));
        }
        body.push_str("   </rdf:Bag></dc:subject>\n");
    }

    format!(
        "<?xpacket begin=\"\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"MacFly\">\n \
<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  \
<rdf:Description rdf:about=\"\"\n    \
xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n    \
xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n    \
xmlns:photoshop=\"http://ns.adobe.com/photoshop/1.0/\"\n    \
xmlns:exif=\"http://ns.adobe.com/exif/1.0/\"{attrs}>\n{body}  </rdf:Description>\n \
</rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PhotoItem {
        PhotoItem {
            id: "ABC/L0/001".to_string(),
            filename: "IMG_0001.HEIC".to_string(),
            title: Some("Rechnung <Mai> & Co".to_string()),
            description: Some("Beleg \"42\"".to_string()),
            date: Some("2024-05-01T10:00:00.000Z".to_string()),
            keywords: vec!["Beleg".to_string(), "2024".to_string()],
            favorite: true,
            width: Some(4032),
            height: Some(3024),
            size_bytes: Some(2_000_000),
            latitude: Some(52.5200),
            longitude: Some(-13.4050),
            altitude: Some(34.5),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "macfly-photos-test-{name}-{}-{}",
            std::process::id(),
            macfly_core::plan::now_unix()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn xmp_contains_escaped_fields_and_gps() {
        let xmp = xmp_for(&sample());
        assert!(xmp.contains("Rechnung &lt;Mai&gt; &amp; Co"));
        assert!(xmp.contains("Beleg &quot;42&quot;"));
        assert!(xmp.contains("<rdf:li>Beleg</rdf:li>"));
        assert!(xmp.contains("xmp:CreateDate=\"2024-05-01T10:00:00.000Z\""));
        assert!(xmp.contains("exif:GPSLatitude=\"52,31.2000N\""));
        assert!(xmp.contains("exif:GPSLongitude=\"13,24.3000W\""));
        assert!(xmp.contains("exif:GPSAltitude=\"3450/100\""));
        assert!(xmp.contains("exif:GPSAltitudeRef=\"0\""));
    }

    #[test]
    fn xmp_without_optional_fields_is_still_well_formed() {
        let mut item = sample();
        item.title = None;
        item.description = None;
        item.keywords.clear();
        item.date = None;
        item.latitude = None;
        item.longitude = None;
        let xmp = xmp_for(&item);
        assert!(!xmp.contains("dc:title"));
        assert!(!xmp.contains("GPSLatitude"));
        assert!(xmp.contains("</x:xmpmeta>"));
    }

    #[test]
    fn place_file_never_overwrites_and_detects_identical_files() {
        let dest = temp_dir("place");
        let a = dest.join(".a.tmp");
        fs::write(&a, b"AAAA").unwrap();
        let (p1, new1) = place_file(&a, &dest, "IMG.jpg").unwrap();
        assert!(new1);
        assert_eq!(p1, dest.join("IMG.jpg"));

        // Gleicher Name, anderer Inhalt: Suffix statt Überschreiben.
        let b = dest.join(".b.tmp");
        fs::write(&b, b"BBBB").unwrap();
        let (p2, new2) = place_file(&b, &dest, "IMG.jpg").unwrap();
        assert!(new2);
        assert_eq!(p2, dest.join("IMG-1.jpg"));
        assert_eq!(fs::read(dest.join("IMG.jpg")).unwrap(), b"AAAA");

        // Gleicher Name, gleicher Inhalt: schon exportiert, nichts Neues.
        let c = dest.join(".c.tmp");
        fs::write(&c, b"AAAA").unwrap();
        let (p3, new3) = place_file(&c, &dest, "IMG.jpg").unwrap();
        assert!(!new3);
        assert_eq!(p3, dest.join("IMG.jpg"));
        assert!(!c.exists());
        let _ = fs::remove_dir_all(&dest);
    }

    #[test]
    fn manifest_uses_shasum_format_with_relative_paths() {
        let dest = temp_dir("manifest");
        let file = dest.join("a.jpg");
        fs::write(&file, b"x").unwrap();
        let hash = sha256_file_hex(&file).unwrap();
        let (path, _) = write_manifest(&dest, 1, &[(file, hash.clone())])
            .unwrap()
            .unwrap();
        let text = fs::read_to_string(path).unwrap();
        assert_eq!(text, format!("{hash}  a.jpg\n"));
        assert!(write_manifest(&dest, 2, &[]).unwrap().is_none());
        let _ = fs::remove_dir_all(&dest);
    }

    #[test]
    fn primary_prefers_still_image_over_live_photo_video() {
        let files = vec![
            PathBuf::from("/x/IMG_1.MOV"),
            PathBuf::from("/x/IMG_1.HEIC"),
        ];
        assert_eq!(pick_primary(&files), Some(PathBuf::from("/x/IMG_1.HEIC")));
    }

    // Die folgenden Tests nutzen das macOS-eigene `sips` mit einem kleinen, selbst erzeugten PNG (8×8).
    #[cfg(target_os = "macos")]
    mod sips {
        use super::*;

        const PNG_8X8: &str = "89504e470d0a1a0a0000000d49484452000000080000000808020000004b6d29dc0000001149444154789c633861648415310c2d090073fb4b0198bb8fc80000000049454e44ae426082";

        fn write_png(dir: &Path, name: &str) -> PathBuf {
            let bytes: Vec<u8> = (0..PNG_8X8.len() / 2)
                .map(|i| u8::from_str_radix(&PNG_8X8[i * 2..i * 2 + 2], 16).unwrap())
                .collect();
            let path = dir.join(name);
            fs::write(&path, bytes).unwrap();
            path
        }

        #[test]
        fn converts_png_to_jpeg_and_shrinks_only_larger_images() {
            let dir = temp_dir("sips");
            let png = write_png(&dir, "IMG_1.png");

            // 8 px > 4 px: wird verkleinert.
            let out = convert_image(
                &png,
                PhotoConvert {
                    format: ImageFormat::Jpeg,
                    max_size: Some(4),
                    quality: Some(80),
                },
            )
            .unwrap()
            .unwrap();
            assert_eq!(out, dir.join("IMG_1.jpg"));
            assert_eq!(sips_dimensions(&out).unwrap(), (4, 4));
            assert_eq!(&fs::read(&out).unwrap()[..2], &[0xFF, 0xD8]);

            // 8 px < 20 px: `sips -Z` würde auf 20 px vergrößern, MacFly lässt das Bild unverändert.
            let png2 = write_png(&dir, "IMG_2.png");
            let out2 = convert_image(
                &png2,
                PhotoConvert {
                    format: ImageFormat::Jpeg,
                    max_size: Some(20),
                    quality: None,
                },
            )
            .unwrap()
            .unwrap();
            assert_eq!(sips_dimensions(&out2).unwrap(), (8, 8));

            // Gleiches Format, nichts zu ändern: keine Umwandlung.
            let none = convert_image(
                &png,
                PhotoConvert {
                    format: ImageFormat::Png,
                    max_size: None,
                    quality: None,
                },
            );
            assert!(none.unwrap().is_none());
            let _ = fs::remove_dir_all(&dir);
        }
    }
}
