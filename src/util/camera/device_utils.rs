//     Copyright (C) 2020-2021l1npengtul
//
//     This program is free software: you can redistribute it and/or modify
//     it under the terms of the GNU General Public License as published by
//     the Free Software Foundation, either version 3 of the License, or
//     (at your option) any later version.
//
//     This program is distributed in the hope that it will be useful,
//     but WITHOUT ANY WARRANTY; without even the implied warranty of
//     MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//     GNU General Public License for more details.
//
//     You should have received a copy of the GNU General Public License
//     along with this program.  If not, see <https://www.gnu.org/licenses/>.

use crate::error::invalid_device_error::InvalidDeviceError;
use crate::util::camera::{
    camera_device::{UVCameraDevice, V4LinuxDevice},
    webcam::Webcam,
};
use gdnative::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering, collections::HashMap, convert::TryFrom, error::Error, fmt::Display,
    os::raw::c_int,
};
use usb_enumeration::USBDevice;
use uvc::{DeviceHandle, FrameFormat, StreamHandle};
use v4l::{device::List, framesize::FrameSizeEnum, prelude::*, FourCC};

#[derive(Clone, Deserialize, Serialize)]
pub struct DeviceDesc {
    pub(crate) vid: Option<c_int>,
    pub(crate) pid: Option<c_int>,
    pub(crate) ser: Option<String>,
}
impl DeviceDesc {
    pub fn new(device: uvc::Device) -> Result<Self, Box<dyn Error>> {
        let device_desc = device.description()?;
        Ok(DeviceDesc {
            vid: Some(c_int::from(device_desc.vendor_id)),
            pid: Some(c_int::from(device_desc.product_id)),
            ser: device_desc.serial_number,
        })
    }
    pub fn from_description(device: uvc::DeviceDescription) -> Self {
        DeviceDesc {
            vid: Some(c_int::from(device.vendor_id)),
            pid: Some(c_int::from(device.product_id)),
            ser: device.serial_number,
        }
    }
    pub fn from_default() -> Self {
        DeviceDesc {
            vid: None,
            pid: None,
            ser: None,
        }
    }
}

#[derive(Clone)]
pub struct DeviceHolder {
    pub id: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub description: String,
    pub serial: Option<String>,
}
impl DeviceHolder {
    pub fn new(
        id: String,
        vendor_id: u16,
        product_id: u16,
        description: String,
        serial: Option<String>,
    ) -> Self {
        DeviceHolder {
            id,
            vendor_id,
            product_id,
            description,
            serial,
        }
    }

    pub fn from_devices(
        usb: &USBDevice,
        uvc: &uvc::Device,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if let Ok(uvc_desc) = uvc.description() {
            if uvc_desc.vendor_id == usb.vendor_id && uvc_desc.product_id == usb.product_id {
                let mut description: String =
                    String::from(format!("{}:{}", uvc_desc.vendor_id, uvc_desc.product_id));
                let serial = uvc_desc.serial_number.clone();
                if let Some(descript) = usb.description.clone() {
                    description = String::from(format!("{} {}", description, descript));
                }
                let device: DeviceHolder = DeviceHolder::new(
                    usb.id.clone(),
                    uvc_desc.vendor_id,
                    uvc_desc.product_id,
                    description,
                    serial,
                );
                return Ok(device);
            }
        }
        return Err(Box::new(InvalidDeviceError::CannotFindDevice));
    }
}

impl PartialEq for DeviceHolder {
    fn eq(&self, other: &Self) -> bool {
        if self.description == other.description
            && self.product_id == other.product_id
            && self.vendor_id == other.vendor_id
            && self.id == other.id
        {
            return false;
        }
        true
    }
}

#[derive(Copy, Clone, Eq, Hash)]
pub struct Resolution {
    pub x: u32,
    pub y: u32,
}

impl Resolution {
    pub fn from_variant(var: Variant) -> Result<Self, ()> {
        if let Some(v) = var.try_to_vector2() {
            if v.x <= 0.0 || v.y <= 0.0 {
                return Err(());
            } else {
                let x = v.x as u32;
                let y = v.y as u32;
                return Ok(Resolution { x, y });
            }
        }
        Err(())
    }
}

impl TryFrom<v4l::framesize::FrameSize> for Resolution {
    type Error = String;

    fn try_from(value: v4l::framesize::FrameSize) -> Result<Self, Self::Error> {
        Ok(match value.size {
            FrameSizeEnum::Stepwise(step) => Resolution {
                x: step.max_width,
                y: step.max_height,
            },
            FrameSizeEnum::Discrete(dis) => Resolution {
                x: dis.width,
                y: dis.height,
            },
        })
    }
}

impl PartialEq for Resolution {
    fn eq(&self, other: &Self) -> bool {
        if self.x == other.x && self.y == other.y {
            return false;
        }
        true
    }
}

impl Display for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.x, self.y)
    }
}

impl PartialOrd for Resolution {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let total_self = self.x;
        let total_other = other.x;
        if total_self > total_other {
            return Some(Ordering::Greater);
        } else if total_self < total_other {
            return Some(Ordering::Less);
        } else {
            return Some(Ordering::Equal);
        }
    }
}
#[derive(Copy, Clone)]
pub enum DeviceFormat {
    YUYV,
    MJPEG,
}

impl DeviceFormat {
    pub fn from_variant(var: Variant) -> Result<Self, ()> {
        if let Some(st) = var.try_to_string() {
            match &st.to_lowercase().to_owned()[..] {
                "yuyv" => {
                    return Ok(DeviceFormat::YUYV);
                }
                "mjpg" | "mjpeg" => {
                    return Ok(DeviceFormat::MJPEG);
                }
                _ => return Err(()),
            }
        }
        Err(())
    }
}

impl Display for DeviceFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceFormat::YUYV => {
                write!(f, "YUYV")
            }
            DeviceFormat::MJPEG => {
                write!(f, "MJPG")
            }
        }
    }
}

pub enum StreamType<'a> {
    V4L2Stream(MmapStream<'a>),
    UVCStream(DeviceHandle<'a>),
}
#[derive(Clone)]
pub enum PossibleDevice {
    UVCAM {
        vendor_id: Option<u16>,
        product_id: Option<u16>,
        serial: Option<String>,
        res: Resolution,
        fps: u32,
        fmt: FrameFormat,
    },
    V4L2 {
        location: PathIndex,
        res: Resolution,
        fps: u32,
        fmt: FourCC,
    },
}

impl PossibleDevice {
    pub fn from_cached_device(
        cached: &CachedDevice,
        res: Resolution,
        fps: u32,
        fmt: DeviceFormat,
    ) -> PossibleDevice {
        match &cached.device_location {
            DeviceContact::UVCAM {
                vendor_id,
                product_id,
                serial,
            } => {
                let dev_format = match fmt {
                    DeviceFormat::YUYV => FrameFormat::YUYV,
                    DeviceFormat::MJPEG => FrameFormat::MJPEG,
                };
                PossibleDevice::UVCAM {
                    vendor_id: vendor_id.to_owned(),
                    product_id: product_id.to_owned(),
                    serial: serial.to_owned(),
                    res,
                    fps,
                    fmt: dev_format,
                }
            }
            DeviceContact::V4L2 { location } => {
                let dev_format = match fmt {
                    DeviceFormat::YUYV => FourCC::new(b"MJPG"),
                    DeviceFormat::MJPEG => FourCC::new(b"YUYV"),
                };
                let lc = match location {
                    PathIndex::Path(p) => PathIndex::Path(p.to_owned()),
                    PathIndex::Index(i) => PathIndex::Index(i.to_owned()),
                };
                PossibleDevice::V4L2 {
                    location: lc,
                    res,
                    fps,
                    fmt: dev_format,
                }
            }
        }
    }

    pub fn to_device_contact(&self) -> DeviceContact {
        return match self {
            PossibleDevice::UVCAM {
                vendor_id,
                product_id,
                serial,
                res,
                fps,
                fmt,
            } => DeviceContact::UVCAM {
                vendor_id: vendor_id.clone(),
                product_id: product_id.clone(),
                serial: serial.clone(),
            },
            PossibleDevice::V4L2 {
                location,
                res,
                fps,
                fmt,
            } => DeviceContact::V4L2 {
                location: location.clone(),
            },
        };
    }
}

#[derive(Clone)]
pub enum PathIndex {
    Path(String),
    Index(usize),
}

#[derive(Clone)]
pub enum DeviceContact {
    UVCAM {
        vendor_id: Option<u16>,
        product_id: Option<u16>,
        serial: Option<String>,
    },
    V4L2 {
        location: PathIndex,
    },
}
impl From<PossibleDevice> for DeviceContact {
    fn from(value: PossibleDevice) -> Self {
        match value {
            PossibleDevice::UVCAM {
                vendor_id,
                product_id,
                serial,
                res,
                fps,
                fmt,
            } => DeviceContact::UVCAM {
                vendor_id,
                product_id,
                serial,
            },
            PossibleDevice::V4L2 {
                location,
                res,
                fps,
                fmt,
            } => DeviceContact::V4L2 { location },
        }
    }
}

#[derive(Clone)]
pub struct CachedDevice {
    device_name: String,
    device_location: DeviceContact,
    device_format_mjpg: Box<HashMap<Resolution, Vec<u32>>>,
    device_format_yuyv: Box<HashMap<Resolution, Vec<u32>>>,
}

impl CachedDevice {
    pub fn from_webcam(camera: Box<dyn Webcam>) -> Result<Self, ()> {
        let device_name = camera.name();
        let device_location = DeviceContact::from(camera.get_inner());
        let resolutions = match camera.get_supported_resolutions() {
            Ok(res) => res,
            Err(_) => return Err(()),
        };

        let mut fmt_res_mjpg: HashMap<Resolution, Vec<u32>> = HashMap::new();
        let mut fmt_res_yuyv: HashMap<Resolution, Vec<u32>> = HashMap::new();

        let mut has_yuyv = false;
        let mut has_mjpg = false;

        // let mut framerate_list: Vec<u32> = Vec::new();

        for res in resolutions {
            has_yuyv = false;
            has_mjpg = false;

            match camera.get_supported_formats(res) {
                Ok(fmt) => {
                    for dev_fmt in fmt {
                        match dev_fmt {
                            DeviceFormat::YUYV => has_yuyv = true,
                            DeviceFormat::MJPEG => has_mjpg = true,
                        }
                    }
                }
                Err(_) => {}
            }

            if has_yuyv && has_mjpg {
                if let Ok(framerates) = camera.get_supported_framerate(res) {
                    fmt_res_mjpg.insert(res, framerates.clone());
                    fmt_res_yuyv.insert(res, framerates.clone());
                }
            }
        }
        Ok(Self {
            device_name,
            device_location,
            device_format_yuyv: Box::new(fmt_res_yuyv),
            device_format_mjpg: Box::new(fmt_res_mjpg),
        })
    }

    pub fn get_name(&self) -> String {
        self.device_name.clone()
    }

    pub fn get_location(&self) -> DeviceContact {
        self.device_location.clone()
    }

    pub fn get_supported_yuyv(&self) -> Box<HashMap<Resolution, Vec<u32>>> {
        self.device_format_yuyv.clone()
    }

    pub fn get_supported_mjpg(&self) -> Box<HashMap<Resolution, Vec<u32>>> {
        self.device_format_mjpg.clone()
    }
}

impl PartialEq for CachedDevice {
    fn eq(&self, other: &Self) -> bool {
        if self.device_name == other.device_name {
            return true;
        }
        false
    }
}

pub fn enumerate_devices() -> Option<HashMap<String, Box<dyn Webcam>>> {
    return match std::env::consts::OS {
        "linux" => {
            let mut known_devices: HashMap<String, Box<dyn Webcam>> = HashMap::new();
            // get device list from v4l2
            for sys_device in List::new() {
                // get device from v4l2 using the index, getting /dev/video0 if it falis
                let v4l_device = match V4LinuxDevice::new(&sys_device.index().unwrap_or(0)) {
                    Ok(dev) => Box::new(dev),
                    Err(_why) => continue,
                };
                // weed out the repeating
                if !known_devices.contains_key(&v4l_device.name()) {
                    known_devices.insert(v4l_device.name(), v4l_device);
                }
            }
            Some(known_devices)
        }
        "macos" | "windows" => {
            let mut known_devices: HashMap<String, Box<dyn Webcam>> = HashMap::new();
            match crate::UVC.devices() {
                Ok(list) => {
                    for uvc_device in list {
                        if let Ok(camera_device) = UVCameraDevice::from_device(uvc_device) {
                            let camera_name = camera_device.name();
                            if !known_devices.contains_key(&camera_name) {
                                known_devices.insert(camera_name, Box::new(camera_device));
                            }
                        }
                    }
                }
                Err(_why) => {
                    return None;
                }
            }
            Some(known_devices)
        }
        _ => None,
    };
}

pub fn enumerate_cache_device() -> Option<HashMap<String, CachedDevice>> {
    return match std::env::consts::OS {
        "linux" => {
            let mut known_devices: HashMap<String, CachedDevice> = HashMap::new();
            // get device list from v4l2
            for sys_device in List::new() {
                // get device from v4l2 using the index, getting /dev/video0 if it falis
                let v4l_device = match V4LinuxDevice::new(&sys_device.index().unwrap_or(0)) {
                    Ok(dev) => CachedDevice::from_webcam(Box::new(dev)).unwrap(),
                    Err(_why) => continue,
                };
                let dev_name = v4l_device.get_name();
                // weed out the repeating
                if !known_devices.contains_key(&dev_name) {
                    known_devices.insert(dev_name, v4l_device);
                }
            }
            Some(known_devices)
        }
        "macos" | "windows" => {
            let mut known_devices: HashMap<String, CachedDevice> = HashMap::new();
            match crate::UVC.devices() {
                Ok(list) => {
                    for uvc_device in list {
                        if let Ok(camera_device) = CachedDevice::from_webcam(Box::new(
                            UVCameraDevice::from_device(uvc_device).unwrap(),
                        )) {
                            let dev_name = camera_device.get_name();
                            // weed out the repeating
                            if !known_devices.contains_key(&dev_name) {
                                known_devices.insert(dev_name, camera_device);
                            }
                        }
                    }
                }
                Err(_why) => {
                    return None;
                }
            }
            Some(known_devices)
        }
        _ => None,
    };
}
