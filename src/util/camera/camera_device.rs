//     Open2DH - Open 2D Holo, a program to procedurally animate your face onto an 3D Model.
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

use crate::error::invalid_device_error::InvalidDeviceError::CannotGetProperty;
use crate::{
    error::invalid_device_error::InvalidDeviceError::{
        CannotFindDevice, CannotGetDeviceInfo, CannotGetFrame, CannotOpenStream, CannotSetProperty,
    },
    ret_boxerr,
    util::camera::{
        device_utils::{
            get_os_webcam_index, DeviceContact, DeviceFormat, PathIndex, PossibleDevice, Resolution,
        },
        webcam::{QueryCamera, Webcam, WebcamType},
    },
};
use flume::{Receiver, Sender, TryRecvError};
use opencv::core::Vec3b;
use opencv::videoio::VideoCaptureAPIs::CAP_ANY;
use opencv::videoio::{VideoWriter, CAP_PROP_FOURCC};
use opencv::{
    core::{Mat, MatTrait, MatTraitManual},
    videoio::{
        VideoCapture, VideoCaptureProperties, VideoCaptureTrait, CAP_MSMF, CAP_PROP_FPS,
        CAP_PROP_FRAME_HEIGHT, CAP_PROP_FRAME_WIDTH, CAP_V4L2,
    },
};
use ouroboros::self_referencing;
use std::{
    cell::{Cell, RefCell},
    error::Error,
    mem::size_of,
    mem::MaybeUninit,
    sync::{atomic::AtomicUsize, Arc},
    time::Instant,
};
use usb_enumeration::{enumerate, Filters};
use uvc::{
    ActiveStream, Context, DeviceDescription, DeviceHandle, FormatDescriptor, FrameFormat,
    StreamHandle,
};
use v4l::{
    buffer::Type,
    format::Format,
    fraction::Fraction,
    framesize::FrameSizeEnum,
    io::{mmap::Stream, traits::CaptureStream},
    video::{capture::Parameters, traits::Capture},
    FourCC,
};

type RefCellOption<T> = RefCell<Option<RefCell<T>>>;

// USE set_format for v4l2 device
pub struct V4LinuxDevice<'a> {
    device_type: WebcamType,
    device_format: Cell<DeviceFormat>,
    device_path: PathIndex,
    device_stream: RefCell<Option<RefCell<Stream<'a>>>>,
    // why do i have to wrap this in 2 refcells please option give an option for a mutable reference PLEASE
    pub inner: RefCell<v4l::Device>,
    opened: Cell<bool>,
}

impl<'a> V4LinuxDevice<'a> {
    pub fn new(index: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let device = match v4l::Device::new(index) {
            Ok(dev) => dev,
            Err(why) => {
                return Err(Box::new(CannotFindDevice(format!(
                    "{}, idx: {}",
                    why.to_string(),
                    index
                ))));
            }
        };
        let device_type = WebcamType::V4linux2;
        let device_path = PathIndex::Index(index);
        Ok(V4LinuxDevice {
            device_type,
            device_format: Cell::new(DeviceFormat::MJPEG),
            device_path,
            device_stream: RefCell::new(None),
            inner: RefCell::new(device),
            opened: Cell::new(false),
        })
    }
    pub fn new_path(path: String) -> Result<Self, Box<dyn std::error::Error>> {
        let device = match v4l::Device::with_path(path.to_string()) {
            Ok(dev) => dev,
            Err(why) => {
                return Err(Box::new(CannotFindDevice(format!(
                    "{}, path: {}",
                    why.to_string(),
                    path
                ))));
            }
        };
        let device_type = WebcamType::V4linux2;
        let device_path = PathIndex::Path(path);
        Ok(V4LinuxDevice {
            device_type,
            device_format: Cell::new(DeviceFormat::MJPEG),
            device_path,
            device_stream: RefCell::new(None),
            inner: RefCell::new(device),
            opened: Cell::new(false),
        })
    }
    pub fn new_location(location: PathIndex) -> Result<Self, Box<dyn std::error::Error>> {
        match location {
            PathIndex::Path(p) => V4LinuxDevice::new_path(p),
            PathIndex::Index(i) => V4LinuxDevice::new(i.to_owned()),
        }
    }
}

impl<'a> Webcam<'a> for V4LinuxDevice<'a> {
    fn name(&self) -> String {
        let device_name = match self.inner.borrow().query_caps() {
            Ok(capability) => capability.card,
            Err(_why) => "".to_string(),
        };
        device_name
    }

    fn set_resolution(&self, res: &Resolution) -> Result<(), Box<dyn std::error::Error>> {
        let mut v4l2_format = FourCC::new(b"YUYV");
        match self.device_format.get() {
            DeviceFormat::YUYV => {}
            DeviceFormat::MJPEG => {
                v4l2_format = FourCC::new(b"MJPG");
            }
        }
        let fmt = Format::new(res.x, res.y, v4l2_format);
        self.inner.borrow_mut().set_format(&fmt)?;
        Ok(())
    }

    fn set_framerate(&self, fps: &u32) -> Result<(), Box<dyn std::error::Error>> {
        let parameter = Parameters::with_fps(*fps);
        self.inner.borrow_mut().set_params(&parameter)?;
        Ok(())
    }

    fn get_resolution(&self) -> Result<Resolution, Box<dyn std::error::Error>> {
        let device = &*self.inner.borrow();
        match device.format() {
            Ok(fmt) => Ok(Resolution::new(fmt.width, fmt.height)),
            Err(why) => {
                ret_boxerr!(why)
            }
        }
    }

    fn get_framerate(&self) -> Result<u32, Box<dyn std::error::Error>> {
        let device = &*self.inner.borrow();
        match device.params() {
            Ok(param) => {
                dbg!(
                    "num: {}, denom: {}",
                    param.interval.numerator,
                    param.interval.denominator
                );
                Ok((param.interval.numerator) as u32)
            }
            Err(why) => {
                ret_boxerr!(why)
            }
        }
    }

    fn get_camera_type(&self) -> WebcamType {
        self.device_type
    }

    fn open_stream(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.opened.get() == false {
            return match Stream::with_buffers(&*self.inner.borrow_mut(), Type::VideoCapture, 4) {
                Ok(stream) => {
                    *self.device_stream.borrow_mut() = Some(RefCell::new(stream));
                    self.opened.set(true);
                    Ok(())
                }
                Err(why) => Err(Box::new(CannotOpenStream(why.to_string()))),
            };
        }
        Ok(())
    }

    fn get_frame(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        match self.device_stream.try_borrow_mut() {
            Ok(m) => match &*m {
                Some(stream) => {
                    let a = &mut *stream.borrow_mut();
                    match a.next() {
                        Ok(fr) => Ok(fr.0.to_vec()),
                        Err(why) => {
                            ret_boxerr!(why)
                        }
                    }
                }
                None => {
                    ret_boxerr!(CannotGetFrame(
                        "Uninitialized stream! Please call `open_stream` first!".to_string()
                    ))
                }
            },
            Err(why) => {
                ret_boxerr!(why);
            }
        }
    }
}

impl<'a> QueryCamera<'a> for V4LinuxDevice<'a> {
    fn get_supported_resolutions(&self) -> Result<Vec<Resolution>, Box<dyn std::error::Error>> {
        let mut v4l2_format = FourCC::new(b"YUYV");
        match self.device_format.get() {
            DeviceFormat::YUYV => {}
            DeviceFormat::MJPEG => {
                v4l2_format = FourCC::new(b"MJPG");
            }
        }
        return match self.inner.borrow().enum_framesizes(v4l2_format) {
            Ok(formats) => {
                let mut ret: Vec<Resolution> = Vec::new();
                for fs in formats {
                    let compat = match fs.size {
                        FrameSizeEnum::Stepwise(step) => Resolution {
                            x: step.min_width,
                            y: step.min_height,
                        },
                        FrameSizeEnum::Discrete(dis) => Resolution {
                            x: dis.width,
                            y: dis.height,
                        },
                    };
                    ret.push(compat);
                }
                Ok(ret)
            }
            Err(why) => Err(Box::new(CannotGetDeviceInfo {
                prop: "Supported Resolutions".to_string(),
                msg: why.to_string(),
            })),
        };
    }
    fn get_supported_framerate(
        &self,
        res: Resolution,
    ) -> Result<Vec<u32>, Box<dyn std::error::Error>> {
        let mut v4l2_format = FourCC::new(b"YUYV");
        match self.device_format.get() {
            DeviceFormat::YUYV => {}
            DeviceFormat::MJPEG => {
                v4l2_format = FourCC::new(b"MJPG");
            }
        }
        return match self
            .inner
            .borrow()
            .enum_frameintervals(v4l2_format, res.x, res.y)
        {
            Ok(interval) => {
                let mut re_t: Vec<u32> = Vec::new();
                for frame in interval {
                    match frame.interval {
                        v4l::frameinterval::FrameIntervalEnum::Discrete(dis) => {
                            re_t.push(dis.denominator);
                        }
                        v4l::frameinterval::FrameIntervalEnum::Stepwise(step) => {
                            re_t.push(step.min.denominator);
                            re_t.push(step.max.denominator);
                        }
                    }
                }
                Ok(re_t)
            }
            Err(why) => Err(Box::new(CannotGetDeviceInfo {
                prop: "Supported Framerates".to_string(),
                msg: why.to_string(),
            })),
        };
    }

    fn get_location(&self) -> DeviceContact {
        DeviceContact::V4L2 {
            location: (self.device_path).clone(),
        }
    }
}

// If you are getting linter errors about how `'this` isn't defined/a valid lifetime,
// ignore it, the macro expansion isn't working properly.
// This crashes. Just use OpenCV
#[self_referencing(chain_hack, pub_extras)]
pub struct UVCameraDevice<'a> {
    device_type: Box<WebcamType>,
    device_desc: Box<String>,
    device_format: Box<Cell<DeviceFormat>>,
    device_resolution: Box<Cell<Option<Resolution>>>,
    device_framerate: Box<Cell<Option<u32>>>,
    device_receiver: Box<Receiver<Vec<u8>>>,
    device_sender: Box<Sender<Vec<u8>>>,
    opened: Box<Cell<bool>>,
    str: Box<&'a str>, // Im too lazy to use PhantomData, so here is a lifetime box &str.
    ctx: Box<Context<'static>>,
    #[borrows(ctx)]
    #[not_covariant]
    device: Box<uvc::Device<'this>>,
    #[borrows(device)]
    #[not_covariant]
    device_handle: Box<DeviceHandle<'this>>,
    #[borrows(device_handle)]
    #[not_covariant]
    stream_handle: Box<RefCell<MaybeUninit<StreamHandle<'this>>>>,
    #[borrows(stream_handle)]
    #[not_covariant]
    active_stream: Box<RefCell<MaybeUninit<ActiveStream<'this, Arc<AtomicUsize>>>>>,
}

impl<'a> UVCameraDevice<'a> {
    pub fn new_camera(
        vendor_id: Option<i32>,
        product_id: Option<i32>,
        serial_number: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let device_name = {
            let ctx = match Context::new() {
                Ok(c) => Box::new(c),
                Err(why) => ret_boxerr!(why),
            };
            let device = match ctx.find_device(vendor_id, product_id, serial_number.as_deref()) {
                Ok(dev) => dev,
                Err(why) => ret_boxerr!(why),
            };
            let description = device.description().unwrap();
            let devices_list = enumerate()
                .with_vendor_id(description.vendor_id)
                .with_product_id(description.product_id);
            let usb_dev = devices_list.get(0).unwrap();
            let device_name = format!(
                "{}:{} {}",
                description.vendor_id,
                description.product_id,
                usb_dev
                    .description
                    .clone()
                    .unwrap_or_else(|| String::from(""))
            );
            device_name
        };

        let result: std::thread::Result<UVCameraDevice> = std::panic::catch_unwind(|| {
            let (send, recv) = {
                let (s, r) = flume::unbounded();
                let send: Box<Sender<Vec<u8>>> = Box::new(s);
                let recv: Box<Receiver<Vec<u8>>> = Box::new(r);
                (send, recv)
            };
            UVCameraDeviceBuilder {
                device_type: Box::new(WebcamType::USBVideo),
                device_desc: Box::new(device_name),
                device_format: Box::new(Cell::new(DeviceFormat::MJPEG)),
                device_resolution: Box::new(Cell::new(None)),
                device_framerate: Box::new(Cell::new(None)),
                device_receiver: recv,
                device_sender: send,
                opened: Box::new(Cell::new(false)),
                str: Box::new("a"),
                ctx: Box::new(Context::new().unwrap()),
                device_builder: |ctx| {
                    Box::new(
                        ctx.find_device(vendor_id, product_id, serial_number.as_deref())
                            .unwrap(),
                    )
                },
                device_handle_builder: |device_builder| Box::new(device_builder.open().unwrap()),
                stream_handle_builder: |_device_handle_builder| {
                    Box::new(RefCell::new(MaybeUninit::uninit()))
                },
                active_stream_builder: |_stream_handle_builder| {
                    Box::new(RefCell::new(MaybeUninit::uninit()))
                },
            }
            .build()
        });
        match result {
            Ok(uvcam) => Ok(uvcam),
            Err(_) => {
                ret_boxerr!(CannotFindDevice(
                    "Failed to build final UVCameraStruct from builder!".to_string()
                ))
            }
        }
    }

    pub fn from_device(uvc_dev: uvc::Device<'a>) -> Result<Self, Box<dyn std::error::Error>> {
        let description = {
            // block so we make sure parameter is drop
            let a = match uvc_dev.description() {
                Ok(desc) => desc,
                Err(why) => ret_boxerr!(why),
            };
            std::mem::drop(uvc_dev);
            a
        };
        let devices_list = enumerate()
            .with_vendor_id(description.vendor_id)
            .with_product_id(description.product_id);

        let device_name = format!(
            "{}:{} {}",
            description.vendor_id,
            description.product_id,
            devices_list
                .get(0)
                .unwrap()
                .description
                .clone()
                .unwrap_or_else(|| String::from(""))
        );

        let result: std::thread::Result<UVCameraDevice> = std::panic::catch_unwind(|| {
            let (send, recv) = {
                let (s, r) = flume::unbounded();
                let send: Box<Sender<Vec<u8>>> = Box::new(s);
                let recv: Box<Receiver<Vec<u8>>> = Box::new(r);
                (send, recv)
            };
            UVCameraDeviceBuilder {
                device_type: Box::new(WebcamType::USBVideo),
                device_desc: Box::new(device_name),
                device_format: Box::new(Cell::new(DeviceFormat::MJPEG)),
                device_resolution: Box::new(Cell::new(None)),
                device_framerate: Box::new(Cell::new(None)),
                device_receiver: recv,
                device_sender: send,
                opened: Box::new(Cell::new(false)),
                str: Box::new("a"),
                ctx: Box::new(Context::new().unwrap()),
                device_builder: |ctx| {
                    Box::new(
                        ctx.find_device(
                            Some(i32::from(description.vendor_id)),
                            Some(i32::from(description.product_id)),
                            description.serial_number.as_deref(),
                        )
                        .unwrap(),
                    )
                },
                device_handle_builder: |device_builder| Box::new(device_builder.open().unwrap()),
                stream_handle_builder: |_device_handle_builder| {
                    Box::new(RefCell::new(MaybeUninit::uninit()))
                },
                active_stream_builder: |_stream_handle_builder| {
                    Box::new(RefCell::new(MaybeUninit::uninit()))
                },
            }
            .build()
        });
        match result {
            Ok(uvcam) => Ok(uvcam),
            Err(_) => {
                ret_boxerr!(CannotFindDevice(
                    "Failed to build final UVCameraStruct from builder!".to_string()
                ))
            }
        }
    }
}

impl<'a> Webcam<'a> for UVCameraDevice<'a> {
    fn name(&self) -> String {
        self.with_device_desc(|name| name).to_string()
    }

    fn set_resolution(&self, res: &Resolution) -> Result<(), Box<dyn std::error::Error>> {
        self.with_device_resolution(|set| set.set(Some(*res)));
        Ok(())
    }

    fn set_framerate(&self, fps: &u32) -> Result<(), Box<dyn std::error::Error>> {
        self.with_device_framerate(|set| set.set(Some(*fps)));
        Ok(())
    }

    fn get_resolution(&self) -> Result<Resolution, Box<dyn Error>> {
        match self.with_device_resolution(|res| res.get()) {
            Some(r) => Ok(r),
            None => ret_boxerr!(CannotGetProperty(
                "Resolution of UVCameraDevice".to_string()
            )),
        }
    }

    fn get_framerate(&self) -> Result<u32, Box<dyn Error>> {
        match self.with_device_framerate(|fps| fps.get()) {
            Some(r) => Ok(r),
            None => ret_boxerr!(CannotGetProperty("Framerate of UVCameraDevice".to_string())),
        }
    }

    fn get_camera_type(&self) -> WebcamType {
        **self.with_device_type(|dev_type| dev_type)
    }

    fn open_stream(&self) -> Result<(), Box<dyn std::error::Error>> {
        // check if we already did this
        let opened: bool = self.with_opened(|o| o.get());
        if opened == true {
            return Ok(());
        }
        // drop active stream
        self.with_active_stream(|astream| unsafe {
            (&mut *astream.borrow_mut()).assume_init_drop();
        });

        self.with(|fields| {
            let resolution: Resolution = self.with_device_resolution(|res| res).get().unwrap();
            let fps: u32 = self.with_device_framerate(|f| f).get().unwrap();
            let devh = fields.device_handle;
            let stream_handle = devh
                .get_stream_handle_with_format_size_and_fps(
                    FrameFormat::MJPEG,
                    resolution.x,
                    resolution.y,
                    fps,
                )
                .unwrap();
            let mut streamhandle_init = MaybeUninit::<StreamHandle>::uninit();
            *fields.stream_handle.borrow_mut() = unsafe {
                streamhandle_init.as_mut_ptr().write(stream_handle);
                streamhandle_init
            }
        });

        // this is cursedstr: Box::new("a"); and forever will be cursed with lifetime errors
        self.with(|fields| {
            let cnt = Arc::new(AtomicUsize::new(0));
            let sender: Sender<Vec<u8>> = *(self.with_device_sender(|send| send)).clone();
            let mut streamh_ref = unsafe {
                let raw_ptr =
                    (*fields.stream_handle.borrow_mut()).as_ptr() as *mut MaybeUninit<StreamHandle>;
                let assume_inited: *mut MaybeUninit<StreamHandle<'static>> =
                    raw_ptr as *mut MaybeUninit<StreamHandle<'static>>;
                &mut *assume_inited
            };
            let mut streamh_init = unsafe { streamh_ref.assume_init_mut() };

            let act_stream = streamh_init
                .start_stream(
                    move |frame, _count| {
                        let vec_frame: Vec<u8> = frame.to_rgb().unwrap().to_bytes().to_vec();
                        sender.send(vec_frame);
                    },
                    cnt,
                )
                .unwrap();
            let mut activestream_init = MaybeUninit::<ActiveStream<Arc<AtomicUsize>>>::uninit();
            *fields.active_stream.borrow_mut() = unsafe {
                activestream_init.as_mut_ptr().write(act_stream);
                activestream_init
            }
        });
        self.with_opened(|o| o.set(true));
        Ok(())
    }

    fn get_frame(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let frame: Result<Vec<u8>, TryRecvError> =
            self.with_device_receiver(|recv| match recv.try_recv() {
                Ok(v) => Ok(v),
                Err(why) => Err(why),
            });
        match frame {
            Ok(v) => Ok(v),
            Err(why) => {
                ret_boxerr!(why)
            }
        }
    }
}

impl<'a> QueryCamera<'a> for UVCameraDevice<'a> {
    fn get_supported_resolutions(&self) -> Result<Vec<Resolution>, Box<dyn std::error::Error>> {
        self.with_device(|device| match device.open() {
            Ok(handler) => {
                let mut resolutions: Vec<Resolution> = Vec::new();
                for format in handler.supported_formats() {
                    for frame in format.supported_formats() {
                        let resolution_string = Resolution {
                            x: u32::from(frame.width()),
                            y: u32::from(frame.height()),
                        };
                        if !resolutions.contains(&resolution_string) {
                            resolutions.push(resolution_string);
                        }
                    }
                }
                Ok(resolutions)
            }
            Err(why) => {
                let a: Box<dyn std::error::Error> = Box::new(CannotGetDeviceInfo {
                    prop: "Supported Resolutions".to_string(),
                    msg: why.to_string(),
                });
                Err(a)
            }
        })
    }

    fn get_supported_framerate(
        &self,
        _res: Resolution,
    ) -> Result<Vec<u32>, Box<dyn std::error::Error>> {
        self.with_device(|device| match device.open() {
            Ok(handler) => {
                let formats: Vec<FormatDescriptor> =
                    handler.supported_formats().into_iter().collect();
                let mut framerates: Vec<u32> = Vec::new();
                for fmt_desc in formats {
                    for frame_desc in fmt_desc.supported_formats() {
                        let mut fps_from_arr: Vec<u32> = frame_desc
                            .intervals_duration()
                            .into_iter()
                            .map(|duration| (1000 / duration.as_millis()) as u32)
                            .collect();
                        framerates.append(&mut fps_from_arr);
                    }
                }
                Ok(framerates)
            }
            Err(why) => {
                let a: Box<dyn std::error::Error> = Box::new(CannotGetDeviceInfo {
                    prop: "Supported Framerates".to_string(),
                    msg: why.to_string(),
                });
                Err(a)
            }
        })
    }

    fn get_location(&self) -> DeviceContact {
        let desc: uvc::Result<DeviceDescription> = self.with_device(|dev| dev.description());

        match desc {
            Ok(description) => DeviceContact::UVCAM {
                vendor_id: Some(description.vendor_id),
                product_id: Some(description.product_id),
                serial: description.serial_number,
            },
            Err(_why) => DeviceContact::UVCAM {
                vendor_id: None,
                product_id: None,
                serial: None,
            },
        }
    }
}

pub struct OpenCVCameraDevice {
    name: RefCell<String>,
    res: Cell<Resolution>,
    fps: Cell<u32>,
    index: Cell<u32>,
    video_capture: RefCell<VideoCapture>,
}

impl OpenCVCameraDevice {
    pub fn new(
        name: String,
        idx: u32,
        frame: u32,
        resolution: Resolution,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let video_capture = {
            // generate video capture with auto detect backend
            let mut v_cap = match VideoCapture::new(idx as i32, get_api_pref_int() as i32) {
                Ok(vc) => vc,
                Err(why) => ret_boxerr!(why),
            };

            if let Err(why) = set_property_res(&mut v_cap, resolution) {
                return Err(why);
            }
            if let Err(why) = set_property_fps(&mut v_cap, frame) {
                return Err(why);
            }
            if let Err(why) = set_property_fourcc(&mut v_cap) {
                return Err(why);
            }

            RefCell::new(v_cap)
        };

        Ok(OpenCVCameraDevice {
            name: RefCell::new(name),
            res: Cell::new(resolution),
            fps: Cell::new(frame),
            index: Cell::new(idx),
            video_capture,
        })
    }

    pub fn from_possible_device(
        n: String,
        possible_device: PossibleDevice,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let name = RefCell::new(n);
        let res = Cell::new(possible_device.res());
        let fps = Cell::new(possible_device.fps());

        let idx = match get_os_webcam_index(possible_device) {
            Ok(i) => i,
            Err(why) => return Err(why),
        };

        let video_capture = {
            // generate video capture with auto detect backend
            let mut v_cap = match VideoCapture::new(idx as i32, get_api_pref_int() as i32) {
                Ok(vc) => vc,
                Err(why) => ret_boxerr!(why),
            };

            if let Err(why) = set_property_res(&mut v_cap, res.get()) {
                return Err(why);
            }
            if let Err(why) = set_property_fps(&mut v_cap, fps.get()) {
                return Err(why);
            }
            if let Err(why) = set_property_fourcc(&mut v_cap) {
                return Err(why);
            }

            RefCell::new(v_cap)
        };

        Ok(OpenCVCameraDevice {
            name,
            res,
            fps,
            index: Cell::new(idx),
            video_capture,
        })
    }

    pub fn from_device_contact(
        n: String,
        device_contact: DeviceContact,
        resolution: Resolution,
        framerate: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        match device_contact {
            DeviceContact::UVCAM {
                vendor_id,
                product_id,
                serial,
            } => {
                let pd = PossibleDevice::UVCAM {
                    vendor_id,
                    product_id,
                    serial,
                    res: resolution,
                    fps: framerate,
                    fmt: FrameFormat::MJPEG,
                };
                OpenCVCameraDevice::from_possible_device(n, pd)
            }
            DeviceContact::V4L2 { location } => {
                let pd = PossibleDevice::V4L2 {
                    location,
                    res: resolution,
                    fps: framerate,
                    fmt: FourCC::new(b"MJPG"),
                };
                OpenCVCameraDevice::from_possible_device(n, pd)
            }
            DeviceContact::OPENCV { index } => {
                OpenCVCameraDevice::new("OpenCVCamera".to_string(), index, framerate, resolution)
            }
        }
    }

    fn res(&self) -> Resolution {
        let a = self
            .video_capture
            .borrow()
            .get(CAP_PROP_FRAME_HEIGHT)
            .unwrap();
        let b = self
            .video_capture
            .borrow()
            .get(CAP_PROP_FRAME_WIDTH)
            .unwrap();
        Resolution::new(a as u32, b as u32)
    }

    fn fps(&self) -> u32 {
        let fps = self.video_capture.borrow().get(CAP_PROP_FPS).unwrap();
        dbg!("{}", fps);
        fps as u32
    }

    pub fn idx(&self) -> u32 {
        self.index.get()
    }

    fn set_res(&self, new_res: Resolution) -> Result<(), Box<dyn std::error::Error>> {
        {
            // make sure we drop the edit lock
            self.res.set(new_res);
        }
        return match self.video_capture.try_borrow_mut() {
            Ok(mut vc) => {
                let v_dev = &mut *vc;
                set_property_res(v_dev, self.res.get())
            }
            Err(why) => ret_boxerr!(why),
        };
    }

    fn set_fps(&self, frame: u32) -> Result<(), Box<dyn std::error::Error>> {
        {
            self.fps.set(frame);
        }
        return match self.video_capture.try_borrow_mut() {
            Ok(mut vc) => {
                let v_dev = &mut *vc;
                set_property_fps(v_dev, frame)
            }
            Err(why) => ret_boxerr!(why),
        };
    }

    fn open_stream_inner(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.video_capture.borrow().is_opened().unwrap_or(false) {
            ret_boxerr!(CannotOpenStream("Cannot Open OPENCV stream!".to_string()))
        }
        Ok(())
    }

    fn get_next_frame(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let vc = &mut *self.video_capture.borrow_mut();
        {
            let mut frame = Mat::default().unwrap();
            match vc.read(&mut frame) {
                Ok(_) => {}
                Err(why) => {
                    ret_boxerr!(why);
                }
            }

            if frame.size().unwrap().width > 0 {
                if frame.is_continuous().unwrap() {
                    // use a memcpy - we about to get f u n k y
                    let mut ret_vec: Vec<u8> = Vec::new();
                    ret_vec.reserve_exact(
                        (frame.rows() * frame.cols() * frame.channels().unwrap_or(3)) as usize,
                    );
                    // make a scope so the vec outlives the pointer 100%
                    {
                        let vec_ptr = ret_vec.as_mut_ptr().cast(); // this looks, feels, and is probably a sin.
                        let mat_ptr = frame.as_raw_Mat();
                        unsafe {
                            mat_ptr.copy_to_nonoverlapping(
                                vec_ptr,
                                (size_of::<u8>() as i32
                                    * frame.rows()
                                    * frame.cols()
                                    * frame.channels().unwrap_or(3))
                                    as usize,
                            );
                        }
                    }
                    return Ok(ret_vec);
                }
                // non continuous mat
                // TODO: Fix
                let mut ret_vec: Vec<u8> = Vec::new();
                ret_vec.reserve_exact(
                    (frame.rows() * frame.cols() * frame.channels().unwrap_or(3)) as usize,
                );
                for row in 0..(frame.rows() - 1) {
                    let mat_rw = match frame.row(row) {
                        Ok(m) => m,
                        Err(why) => {
                            dbg!("{}", why.to_string());
                            ret_boxerr!(why);
                        }
                    };
                    let slice = match mat_rw.data_typed::<Vec3b>() {
                        Ok(sl) => sl,
                        Err(why) => {
                            dbg!("{}", why.to_string());
                            ret_boxerr!(why);
                        }
                    };
                    for px in slice {
                        ret_vec.push(px.0[0]);
                        ret_vec.push(px.0[1]);
                        ret_vec.push(px.0[2]);
                    }
                }
                dbg!("aaa");
                return Ok(ret_vec);
            }
        }
        ret_boxerr!(CannotGetFrame("Unsatisfied Conditions".to_string()))

        // TODO: convert Mat to array/slice/nalgebra matrix
    }

    // hide the body
    // dissolve it in lime,
    // all for a crime
    // of saying "pettan"
    pub fn dispose_of_body(self) {}
}

impl<'a> Webcam<'a> for OpenCVCameraDevice {
    fn name(&self) -> String {
        (*self.name.borrow()).clone()
    }

    fn set_resolution(&self, res: &Resolution) -> Result<(), Box<dyn std::error::Error>> {
        self.set_res(*res)
    }

    fn set_framerate(&self, fps: &u32) -> Result<(), Box<dyn std::error::Error>> {
        self.set_fps(*fps)
    }

    fn get_resolution(&self) -> Result<Resolution, Box<dyn Error>> {
        Ok(self.res())
    }

    fn get_framerate(&self) -> Result<u32, Box<dyn Error>> {
        Ok(self.fps())
    }

    fn get_camera_type(&self) -> WebcamType {
        WebcamType::OpenCVCapture
    }

    fn open_stream(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.open_stream_inner()
    }

    fn get_frame(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        self.get_next_frame()
    }
}

fn set_property_res(
    vc: &mut VideoCapture,
    res: Resolution,
) -> Result<(), Box<dyn std::error::Error>> {
    match vc.set(
        VideoCaptureProperties::CAP_PROP_FRAME_HEIGHT as i32,
        f64::from(res.y),
    ) {
        Ok(r) => {
            if !r {
                ret_boxerr!(CannotSetProperty("CAP_PROP_FRAME_HEIGHT".to_string()))
            }
        }
        Err(why) => {
            return Err(Box::new(why));
        }
    }

    match vc.set(
        VideoCaptureProperties::CAP_PROP_FRAME_WIDTH as i32,
        f64::from(res.x),
    ) {
        Ok(r) => {
            if !r {
                ret_boxerr!(CannotSetProperty("CAP_PROP_FRAME_WIDTH".to_string()))
            }
        }
        Err(why) => {
            ret_boxerr!(why)
        }
    }

    Ok(())
}

fn set_property_fps(vc: &mut VideoCapture, fps: u32) -> Result<(), Box<dyn std::error::Error>> {
    match vc.set(VideoCaptureProperties::CAP_PROP_FPS as i32, f64::from(fps)) {
        Ok(r) => {
            if !r {
                ret_boxerr!(CannotSetProperty("CAP_PROP_FPS".to_string()))
            }
        }
        Err(why) => ret_boxerr!(why),
    }
    Ok(())
}

fn set_property_fourcc(vc: &mut VideoCapture) -> Result<(), Box<dyn std::error::Error>> {
    match vc.set(
        CAP_PROP_FOURCC as i32,
        VideoWriter::fourcc('M' as i8, 'J' as i8, 'P' as i8, 'G' as i8).unwrap() as f64,
    ) {
        Ok(r) => {
            if !r {
                ret_boxerr!(CannotSetProperty("CAP_PROP_FOURCC".to_string()));
            }
        }
        Err(why) => {
            ret_boxerr!(why);
        }
    }
    Ok(())
}

fn get_api_pref_int() -> u32 {
    match std::env::consts::OS {
        "linux" => CAP_V4L2 as u32,
        "windows" => CAP_MSMF as u32,
        &_ => CAP_ANY as u32,
    }
}
