//! Small owned bitmaps. Pixels cross worker threads; GDI handles never do.
use std::mem::size_of;
use windows::{
    core::*,
    Win32::{
        Foundation::*, Graphics::Gdi::*, System::LibraryLoader::*, UI::WindowsAndMessaging::*,
    },
};

const MAX_BITMAP_SIDE: i32 = 256;
const BADGE_FILL: u32 = 0xffa6e3a1; // Catppuccin Mocha green.
const BADGE_BORDER: u32 = 0xff1e1e2e; // Catppuccin Mocha base.

pub struct Bitmap(pub HBITMAP);
impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.0.into());
        }
    }
}

fn info(width: i32, height: i32) -> BITMAPINFO {
    BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn bitmap(pixels: &[u32], side: i32) -> Result<Bitmap> {
    if !(1..=MAX_BITMAP_SIDE).contains(&side) || pixels.len() != (side * side) as usize {
        return Err(Error::from_hresult(E_INVALIDARG));
    }
    unsafe {
        let mut bits = std::ptr::null_mut();
        let image = Bitmap(CreateDIBSection(
            None,
            &info(side, side),
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )?);
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast(), pixels.len());
        Ok(image)
    }
}

/// Fit an already bounded Shell bitmap into a transparent square, without cropping.
pub fn pixels(image: &Bitmap, side: i32) -> Result<Vec<u32>> {
    if !(1..=MAX_BITMAP_SIDE).contains(&side) {
        return Err(Error::from_hresult(E_INVALIDARG));
    }
    unsafe {
        let mut object = BITMAP::default();
        if GetObjectW(
            image.0.into(),
            size_of::<BITMAP>() as i32,
            Some((&mut object as *mut BITMAP).cast()),
        ) == 0
            || object.bmWidth <= 0
            || object.bmHeight <= 0
            || object.bmWidth > side
            || object.bmHeight > side
        {
            return Err(Error::from_hresult(E_INVALIDARG));
        }
        let mut source = vec![0u32; (object.bmWidth * object.bmHeight) as usize];
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return Err(Error::from_win32());
        }
        let read = GetDIBits(
            dc,
            image.0,
            0,
            object.bmHeight as u32,
            Some(source.as_mut_ptr().cast()),
            &mut info(object.bmWidth, object.bmHeight),
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(dc);
        if read != object.bmHeight {
            return Err(Error::from_hresult(E_FAIL));
        }
        // Some Shell providers return an opaque RGB bitmap without an alpha channel.
        if source.iter().all(|pixel| pixel >> 24 == 0) {
            source.iter_mut().for_each(|pixel| *pixel |= 0xff000000);
        }
        let mut square = vec![0; (side * side) as usize];
        let x = (side - object.bmWidth) / 2;
        let y = (side - object.bmHeight) / 2;
        for row in 0..object.bmHeight {
            let from = (row * object.bmWidth) as usize;
            let to = ((y + row) * side + x) as usize;
            square[to..to + object.bmWidth as usize]
                .copy_from_slice(&source[from..from + object.bmWidth as usize]);
        }
        Ok(square)
    }
}

/// Use the embedded app icon in the tray; a small outlined dot marks a full bin.
pub fn tray_icon(side: i32, full: bool) -> Result<HICON> {
    if !(1..=MAX_BITMAP_SIDE).contains(&side) {
        return Err(Error::from_hresult(E_INVALIDARG));
    }
    unsafe {
        let base = HICON(
            LoadImageW(
                Some(GetModuleHandleW(None)?.into()),
                PCWSTR(1usize as *const u16),
                IMAGE_ICON,
                side,
                side,
                IMAGE_FLAGS(0),
            )?
            .0,
        );
        if !full {
            return Ok(base);
        }
        let result = (|| {
            let mut details = ICONINFO::default();
            GetIconInfo(base, &mut details)?;
            let color = Bitmap(details.hbmColor);
            let mask = Bitmap(details.hbmMask);
            let mut data = pixels(&color, side)?;
            let radius = (side as f32 * 0.20).max(3.0);
            let center = side as f32 - radius - 0.5;
            for y in 0..side {
                for x in 0..side {
                    let distance =
                        ((x as f32 - center).powi(2) + (y as f32 - center).powi(2)).sqrt();
                    if distance <= radius {
                        data[(y * side + x) as usize] = if distance <= radius - 1.0 {
                            BADGE_FILL
                        } else {
                            BADGE_BORDER
                        };
                    }
                }
            }
            let color = bitmap(&data, side)?;
            CreateIconIndirect(&ICONINFO {
                fIcon: true.into(),
                hbmColor: color.0,
                hbmMask: mask.0,
                ..Default::default()
            })
        })();
        if result.is_ok() {
            let _ = DestroyIcon(base);
        }
        Ok(result.unwrap_or(base))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bitmap_round_trip_preserves_alpha_and_pixels() -> Result<()> {
        let data = [0, 0xff112233, 0x80402010, 0xffffffff];
        let image = bitmap(&data, 2)?;
        assert_eq!(pixels(&image, 2)?, data);
        assert!(bitmap(&data, 0).is_err());
        assert!(pixels(&image, 0).is_err());
        assert!(pixels(&image, MAX_BITMAP_SIDE + 1).is_err());
        let padded = pixels(&image, 4)?;
        assert_eq!(&padded[..4], &[0; 4]);
        assert_eq!(&padded[5..7], &data[..2]);
        assert_eq!(&padded[9..11], &data[2..]);
        Ok(())
    }
    #[test]
    fn branded_tray_icon_has_distinct_full_state() -> Result<()> {
        unsafe {
            for side in [16, 20, 24, 32] {
                let mut states = Vec::new();
                for full in [false, true] {
                    let icon = tray_icon(side, full)?;
                    let mut details = ICONINFO::default();
                    let result = GetIconInfo(icon, &mut details);
                    let _ = DestroyIcon(icon);
                    result?;
                    let color = Bitmap(details.hbmColor);
                    let _mask = Bitmap(details.hbmMask);
                    states.push(pixels(&color, side)?);
                }
                assert_ne!(states[0], states[1], "Tray states must differ at {side}px");
                assert!(
                    states[1].contains(&BADGE_FILL),
                    "Full badge must be visible"
                );
            }
        }
        Ok(())
    }
}
