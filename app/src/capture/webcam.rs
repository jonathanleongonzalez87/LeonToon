//! Proveedor de webcams UVC con nokhwa (la biblioteca que eligió la Fase 0, Issue #58). Se probó con una webcam Trust
//! Full HD: JPEG de cámara a 1920×1080 y unos 30 cuadros por segundo.

use super::*;
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{ApiBackend, CameraFormat, CameraIndex, ControlValueDescription, ControlValueSetter, FrameFormat, KnownCameraControl, KnownCameraControlFlag, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;

pub struct WebcamProvider;

impl CaptureProvider for WebcamProvider {
    fn name(&self) -> &'static str {
        "webcam"
    }

    fn list(&self) -> Result<Vec<DeviceInfo>, String> {
        let list = nokhwa::query(ApiBackend::Auto).map_err(|e| e.to_string())?;
        Ok(list.iter().map(|c| DeviceInfo { id: format!("webcam:{}", c.index()), name: c.human_name(), provider: "webcam".into() }).collect())
    }

    fn open(&self, id: &str) -> Result<Box<dyn CaptureDevice>, String> {
        let index: u32 = id.strip_prefix("webcam:").and_then(|n| n.parse().ok()).ok_or_else(|| format!("identificador de webcam no válido: {id}"))?;
        // Primero se mira qué formatos ofrece y se elige el mayor en JPEG de cámara con al menos 15 cuadros por segundo.
        let probe = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestResolution);
        let mut cam = Camera::new(CameraIndex::Index(index), probe).map_err(|e| e.to_string())?;
        let mut formats: Vec<CameraFormat> = cam.compatible_camera_formats().unwrap_or_default();
        formats.sort_by_key(|f| (std::cmp::Reverse(f.width() * f.height()), std::cmp::Reverse(f.frame_rate())));
        let best = formats.iter().find(|f| f.format() == FrameFormat::MJPEG && f.frame_rate() >= 15).or_else(|| formats.iter().find(|f| f.frame_rate() >= 15)).or(formats.first()).copied();
        if let Some(best) = best {
            let _ = cam.set_camera_requset(RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(best)));
        }
        cam.open_stream().map_err(|e| e.to_string())?;
        Ok(Box::new(Webcam { cam, known: Vec::new() }))
    }
}

struct Webcam {
    cam: Camera,
    known: Vec<(String, KnownCameraControl)>,
}

fn label(c: KnownCameraControl) -> Option<&'static str> {
    use KnownCameraControl::*;
    Some(match c {
        Brightness => "Brillo",
        Contrast => "Contraste",
        Hue => "Tono",
        Saturation => "Saturación",
        Sharpness => "Nitidez",
        Gamma => "Gamma",
        WhiteBalance => "Balance de blancos",
        BacklightComp => "Compensación de contraluz",
        Gain => "Ganancia",
        Pan => "Giro",
        Tilt => "Inclinación",
        Zoom => "Zoom",
        Exposure => "Exposición",
        Iris => "Iris",
        Focus => "Enfoque",
        Other(_) => return None,
    })
}

impl CaptureDevice for Webcam {
    /// Solo los controles que la cámara declara de verdad y que se pueden cambiar.
    fn controls(&mut self) -> Vec<Control> {
        let mut out = Vec::new();
        self.known.clear();
        for c in self.cam.camera_controls().unwrap_or_default() {
            let Some(name) = label(c.control()) else { continue };
            if !c.active() || c.flag().iter().any(|f| matches!(f, KnownCameraControlFlag::ReadOnly | KnownCameraControlFlag::Disabled)) {
                continue;
            }
            let key = format!("{:?}", c.control());
            let control = match c.description() {
                ControlValueDescription::IntegerRange { min, max, value, step, default } if max > min => Control { key: key.clone(), label: name.into(), min: *min, max: *max, step: (*step).max(1), value: *value, default: *default, boolean: false },
                ControlValueDescription::Boolean { value, default } => Control { key: key.clone(), label: name.into(), min: 0, max: 1, step: 1, value: *value as i64, default: *default as i64, boolean: true },
                _ => continue,
            };
            self.known.push((key, c.control()));
            out.push(control);
        }
        out
    }

    fn frame(&mut self) -> Result<RawFrame, String> {
        let f = self.cam.frame().map_err(|e| e.to_string())?;
        let res = f.resolution();
        if f.source_frame_format() == FrameFormat::MJPEG {
            Ok(RawFrame { width: res.width(), height: res.height(), data: f.buffer().to_vec(), jpeg: true })
        } else {
            let img = f.decode_image::<RgbFormat>().map_err(|e| e.to_string())?;
            Ok(RawFrame { width: img.width(), height: img.height(), data: img.into_raw(), jpeg: false })
        }
    }

    fn set_control(&mut self, key: &str, value: i64) -> Result<(), String> {
        let Some((_, id)) = self.known.iter().find(|(k, _)| k == key).cloned() else {
            return Err(format!("este dispositivo no tiene el control {key}"));
        };
        let setter = match self.cam.camera_control(id).map(|c| c.description().clone()) {
            Ok(ControlValueDescription::Boolean { .. }) => ControlValueSetter::Boolean(value != 0),
            _ => ControlValueSetter::Integer(value),
        };
        self.cam.set_camera_control(id, setter).map_err(|e| e.to_string())
    }
}
