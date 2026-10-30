//! Images importées (plan dessiné, photo jointe à l'assistant, image de fond du plan 2D) : tous les
//! formats courants sont convertis en PNG ou JPEG, les seuls que lisent le plan 2D et les services
//! d'IA. HEIC/HEIF (photos d'iPhone) : décodeur intégré, sans les extensions payantes de Windows ;
//! WebP, GIF, BMP, TIFF, ICO, TGA… : bibliothèque `image` ; SVG : resvg ; le reste (AVIF, JPEG XL,
//! fichiers RAW des appareils photo) : les décodeurs installés dans Windows. L'orientation de la
//! photo (appareil tenu en hauteur) est appliquée.

use crate::tr;
use image::{DynamicImage, GenericImageView, ImageFormat, metadata::Orientation};
use std::io::Cursor;
use std::path::Path;

/// Extensions proposées dans la fenêtre « Ouvrir » (les autres fichiers restent ouvrables avec « Tous
/// les fichiers »).
pub const EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "jpe", "jfif", "heic", "heif", "hif", "avif", "webp", "gif", "bmp", "dib", "tif", "tiff", "ico", "tga", "pnm", "pbm", "pgm", "ppm", "qoi", "dds", "hdr", "svg", "jxl", "dng", "cr2", "cr3", "nef", "nrw", "arw", "srf", "sr2", "orf", "rw2", "raf", "pef", "srw", "erf", "3fr", "kdc", "mrw", "x3f",
];

/// Fichier plus lourd : refusé (une photo RAW fait rarement plus de 150 Mo).
const MAX_FILE: u64 = 200 * 1024 * 1024;

/// Image convertie : octets PNG ou JPEG et sa taille.
#[derive(Clone)]
pub struct Picture {
    pub bytes: Vec<u8>,
    /// « image/png » ou « image/jpeg »
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
}

impl Picture {
    /// Extension du fichier à écrire (« png » ou « jpg »).
    pub fn ext(&self) -> &'static str {
        if self.mime == "image/png" { "png" } else { "jpg" }
    }
}

/// Fenêtre « Ouvrir » de Windows pour une image (tous les formats acceptés).
pub fn pick() -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .set_title("ARKLIGHT")
        .add_filter(tr("Image (photo, plan)", "Image (photo, plot)"), EXTENSIONS)
        .add_filter(tr("Tous les fichiers", "All files"), &["*"])
        .pick_file()
}

/// Lit une image de n'importe quel format et la rend en PNG (transparence) ou JPEG (photo), droite
/// et réduite à `max_side` pixels de côté au plus. Un PNG ou un JPEG déjà droit et assez petit est
/// gardé tel quel.
pub fn load(path: &Path, max_side: u32) -> Result<Picture, String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len > MAX_FILE {
        return Err(tr("Image trop lourde (200 Mo maximum)", "Image too large (200 MB maximum)").into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    convert(&bytes, &ext, max_side, Some(path))
}

/// La même image réduite à `max_side` pixels de côté au plus (photo envoyée à l'assistant, tirée
/// de l'image de fond).
pub fn shrink(p: &Picture, max_side: u32) -> Result<Picture, String> {
    if p.width.max(p.height) <= max_side {
        return Ok(p.clone());
    }
    let img = image::load_from_memory(&p.bytes).map_err(|e| e.to_string())?;
    encode(img, max_side)
}

/// [`load`] sur un fil de travail (une grande photo HEIC prend quelques secondes), puis `done` sur
/// le fil de l'interface.
pub fn load_async(path: std::path::PathBuf, max_side: u32, done: impl FnOnce(&crate::AppWindow, &mut crate::State, Result<Picture, String>) + Send + 'static) {
    std::thread::spawn(move || {
        let r = load(&path, max_side);
        crate::ai_panel::on_ui(move |app, st| done(app, st, r));
    });
}

/// [`load`] sur des octets déjà lus (`ext` : extension du fichier, en minuscules, pour les formats
/// sans signature comme le SVG ; `path` : pour les décodeurs de Windows).
pub fn convert(bytes: &[u8], ext: &str, max_side: u32, path: Option<&Path>) -> Result<Picture, String> {
    let unreadable = || -> String {
        if is_heif(bytes) {
            tr("Photo HEIC/HEIF illisible", "Unreadable HEIC/HEIF photo").into()
        } else {
            tr("Format d’image non reconnu ou fichier abîmé", "Unrecognised image format or damaged file").into()
        }
    };
    // PNG ou JPEG droit et assez petit : tel quel (pas de perte en réenregistrant)
    if let Some(format @ (ImageFormat::Png | ImageFormat::Jpeg)) = image::guess_format(bytes).ok()
        && let Ok((w, h, orientation)) = probe(bytes, format)
        && orientation == Orientation::NoTransforms
        && w.max(h) <= max_side
        && w > 0
        && h > 0
    {
        let mime = if format == ImageFormat::Png { "image/png" } else { "image/jpeg" };
        return Ok(Picture { bytes: bytes.to_vec(), mime, width: w, height: h });
    }
    let img = if is_heif(bytes) {
        // HEIC d'iPhone : décodeur intégré ; AVIF (AV1) et variantes : décodeurs de Windows
        decode_heif(bytes).or_else(|e| path.ok_or(e).and_then(decode_system))
    } else if ext == "svg" || looks_like_svg(bytes) {
        decode_svg(bytes, max_side)
    } else {
        decode_image(bytes, ext).or_else(|e| path.ok_or(e).and_then(decode_system))
    };
    let img = img.map_err(|_| unreadable())?;
    if img.width() == 0 || img.height() == 0 {
        return Err(unreadable());
    }
    encode(img, max_side)
}

/// Conteneur HEIF (HEIC, HEIF, AVIF) : boîte « ftyp » avec une marque de la famille.
fn is_heif(bytes: &[u8]) -> bool {
    bytes.len() > 12 && &bytes[4..8] == b"ftyp" && matches!(&bytes[8..12], b"heic" | b"heix" | b"heim" | b"heis" | b"hevc" | b"hevx" | b"hevm" | b"hevs" | b"mif1" | b"msf1" | b"mif2" | b"avif" | b"avis")
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_lowercase();
    head.contains("<svg")
}

/// Taille et orientation (EXIF) d'un PNG ou d'un JPEG, sans décoder les pixels.
fn probe(bytes: &[u8], format: ImageFormat) -> image::ImageResult<(u32, u32, Orientation)> {
    use image::ImageDecoder;
    let mut reader = image::ImageReader::new(Cursor::new(bytes));
    reader.set_format(format);
    let mut decoder = reader.into_decoder()?;
    let (w, h) = decoder.dimensions();
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    Ok((w, h, orientation))
}

/// Formats de la bibliothèque `image`, orientation EXIF appliquée ; ceux sans signature (TGA) sont
/// reconnus à leur extension.
fn decode_image(bytes: &[u8], ext: &str) -> Result<DynamicImage, String> {
    use image::ImageDecoder;
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| e.to_string())?;
    if reader.format().is_none() {
        reader.set_format(ImageFormat::from_extension(ext).ok_or_else(String::new)?);
    }
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    img.apply_orientation(orientation);
    Ok(img)
}

/// HEIC/HEIF codé en HEVC (photos d'iPhone et de nombreux téléphones). Le décodeur applique les
/// rotations et miroirs du fichier (`irot`, `imir`), qui priment sur l'orientation EXIF.
fn decode_heif(bytes: &[u8]) -> Result<DynamicImage, String> {
    use zune_heic::zune_core::bytestream::ZCursor;
    use zune_heic::zune_core::colorspace::ColorSpace;
    let mut decoder = zune_heic::HeifDecoder::new(ZCursor::new(bytes));
    let pixels = decoder.decode().map_err(|e| format!("{e:?}"))?;
    let (w, h) = (decoder.width().unwrap_or(0) as u32, decoder.height().unwrap_or(0) as u32);
    let img = match decoder.colorspace() {
        Some(ColorSpace::RGB) => image::RgbImage::from_raw(w, h, pixels).map(DynamicImage::ImageRgb8),
        Some(ColorSpace::RGBA) => image::RgbaImage::from_raw(w, h, pixels).map(DynamicImage::ImageRgba8),
        Some(ColorSpace::Luma) => image::GrayImage::from_raw(w, h, pixels).map(DynamicImage::ImageLuma8),
        Some(ColorSpace::LumaA) => image::GrayAlphaImage::from_raw(w, h, pixels).map(DynamicImage::ImageLumaA8),
        _ => None,
    };
    img.ok_or_else(|| "HEIF".to_owned())
}

/// SVG dessiné à sa taille (réduite à `max_side`).
fn decode_svg(bytes: &[u8], max_side: u32) -> Result<DynamicImage, String> {
    let text = String::from_utf8_lossy(bytes);
    let (w, h) = arklight_plot::svg_size(&text).ok_or_else(String::new)?;
    // dessin vectoriel : rendu net au plus grand côté permis
    let k = max_side as f64 / w.max(h);
    let (pw, ph) = (((w * k).round() as u32).max(1), ((h * k).round() as u32).max(1));
    let png = arklight_plot::svg_to_png(&text, pw, ph).ok_or_else(String::new)?;
    image::load_from_memory_with_format(&png, ImageFormat::Png).map_err(|e| e.to_string())
}

/// Réduit puis enregistre en PNG (transparence réelle) ou en JPEG (qualité 90).
fn encode(mut img: DynamicImage, max_side: u32) -> Result<Picture, String> {
    if img.width().max(img.height()) > max_side {
        img = img.resize(max_side, max_side, image::imageops::FilterType::Triangle);
    }
    let (width, height) = img.dimensions();
    let transparent = img.color().has_alpha() && img.to_rgba8().pixels().any(|p| p[3] < 255);
    let mut out = Vec::new();
    if transparent {
        img.to_rgba8().write_to(&mut Cursor::new(&mut out), ImageFormat::Png).map_err(|e| e.to_string())?;
        Ok(Picture { bytes: out, mime: "image/png", width, height })
    } else {
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90);
        img.to_rgb8().write_with_encoder(encoder).map_err(|e| e.to_string())?;
        Ok(Picture { bytes: out, mime: "image/jpeg", width, height })
    }
}

/// Décodeurs installés dans Windows (WIC) : AVIF, JPEG XL, RAW des appareils photo… selon les
/// extensions du Microsoft Store présentes. Sur un fil à part (COM initialisé pour lui seul).
#[cfg(windows)]
fn decode_system(path: &Path) -> Result<DynamicImage, String> {
    let path = path.to_path_buf();
    std::thread::spawn(move || {
        use windows::Win32::Foundation::GENERIC_READ;
        use windows::Win32::Graphics::Imaging::*;
        use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
        use windows::Win32::System::Com::*;
        use windows::Win32::System::Variant::VT_UI2;
        use windows::core::{HSTRING, Interface, w};
        // SAFETY : appels COM ordinaires ; le tampon de CopyPixels a la taille demandée
        unsafe {
            if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
                return Err(String::new());
            }
            let decoded = (|| -> windows::core::Result<DynamicImage> {
                let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
                let decoder = factory.CreateDecoderFromFilename(&HSTRING::from(path.as_os_str()), None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)?;
                let frame = decoder.GetFrame(0)?;
                // orientation de la photo (1 : droite)
                let mut orientation = 1u16;
                if let Ok(meta) = frame.GetMetadataQueryReader() {
                    let mut v = PROPVARIANT::default();
                    if meta.GetMetadataByName(w!("System.Photo.Orientation"), &mut v).is_ok() {
                        let inner = &v.Anonymous.Anonymous;
                        if inner.vt == VT_UI2 {
                            orientation = inner.Anonymous.uiVal;
                        }
                    }
                    let _ = PropVariantClear(&mut v);
                }
                let converter = factory.CreateFormatConverter()?;
                converter.Initialize(&frame.cast::<IWICBitmapSource>()?, &GUID_WICPixelFormat32bppRGBA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)?;
                let (mut w, mut h) = (0u32, 0u32);
                converter.GetSize(&mut w, &mut h)?;
                let mut buf = vec![0u8; w as usize * h as usize * 4];
                converter.CopyPixels(std::ptr::null(), w * 4, &mut buf)?;
                let mut img = image::RgbaImage::from_raw(w, h, buf).map(DynamicImage::ImageRgba8).ok_or_else(windows::core::Error::empty)?;
                if let Some(o) = Orientation::from_exif(orientation.min(8) as u8) {
                    img.apply_orientation(o);
                }
                Ok(img)
            })();
            CoUninitialize();
            decoded.map_err(|e| e.message())
        }
    })
    .join()
    .unwrap_or_else(|_| Err(String::new()))
}

#[cfg(not(windows))]
fn decode_system(_: &Path) -> Result<DynamicImage, String> {
    Err(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photo(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x * 7) as u8, (y * 5) as u8, 120])))
    }

    fn bytes_of(img: &DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), format).unwrap();
        out
    }

    #[test]
    fn png_droit_garde_tel_quel() {
        let png = bytes_of(&photo(40, 30), ImageFormat::Png);
        let p = convert(&png, "png", 1600, None).unwrap();
        assert_eq!((p.mime, p.width, p.height), ("image/png", 40, 30));
        assert_eq!(p.bytes, png);
    }

    #[test]
    fn autres_formats_convertis_en_jpeg() {
        for format in [ImageFormat::WebP, ImageFormat::Bmp, ImageFormat::Gif, ImageFormat::Tiff, ImageFormat::Tga] {
            let src = bytes_of(&photo(64, 48), format);
            let p = convert(&src, format.extensions_str()[0], 1600, None).unwrap_or_else(|e| panic!("{format:?} : {e}"));
            assert_eq!((p.mime, p.width, p.height), ("image/jpeg", 64, 48), "{format:?}");
            assert!(p.bytes.starts_with(&[0xFF, 0xD8]));
        }
    }

    #[test]
    fn grande_image_reduite() {
        let src = bytes_of(&photo(3000, 1500), ImageFormat::Png);
        let p = convert(&src, "png", 1600, None).unwrap();
        assert_eq!((p.width, p.height), (1600, 800));
        let small = shrink(&p, 400).unwrap();
        assert_eq!((small.mime, small.width, small.height), ("image/jpeg", 400, 200));
        assert_eq!(shrink(&small, 1600).unwrap().bytes, small.bytes);
    }

    #[test]
    fn transparence_gardee_en_png() {
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(20, 10, |x, _| image::Rgba([200, 10, 10, if x < 10 { 0 } else { 255 }])));
        let p = convert(&bytes_of(&img, ImageFormat::WebP), "webp", 1600, None).unwrap();
        assert_eq!(p.mime, "image/png");
    }

    #[test]
    fn svg_dessine() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect width="200" height="100" fill="#ff8800"/></svg>"##;
        let p = convert(svg, "svg", 1600, None).unwrap();
        assert_eq!((p.width, p.height), (1600, 800));
    }

    #[test]
    fn heif_abime_refuse() {
        let mut heic = vec![0, 0, 0, 24];
        heic.extend_from_slice(b"ftypheic\0\0\0\0mif1heic");
        heic.extend_from_slice(&[0; 64]);
        assert!(is_heif(&heic));
        assert_eq!(convert(&heic, "heic", 1600, None).err().as_deref(), Some("Photo HEIC/HEIF illisible"));
    }

    /// Vraie photo HEIC (pas fournie avec les sources) : ARKLIGHT_TEST_HEIC=chemin cargo test
    #[test]
    fn photo_heic_reelle() {
        let Some(path) = std::env::var_os("ARKLIGHT_TEST_HEIC") else { return };
        let started = std::time::Instant::now();
        let p = load(Path::new(&path), 4096).unwrap();
        eprintln!("HEIC → {} {}×{} ({} Ko) en {:?}", p.mime, p.width, p.height, p.bytes.len() / 1024, started.elapsed());
        assert_eq!(p.mime, "image/jpeg");
        assert!(p.width.max(p.height) <= 4096);
        if let Some(out) = std::env::var_os("ARKLIGHT_TEST_HEIC_OUT") {
            std::fs::write(out, shrink(&p, 800).unwrap().bytes).unwrap();
        }
    }

    #[test]
    fn fichier_inconnu_refuse() {
        assert!(convert(b"pas une image du tout", "txt", 1600, None).is_err());
    }
}
