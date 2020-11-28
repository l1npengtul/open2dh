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


use crate::util::camera::device_utils::*;

pub trait Webcam {
    fn name(&self) -> String;
    fn set_resolution(&mut self, res: Resolution) -> Result<(), Box<dyn std::error::Error>>;
    fn set_framerate(&mut self, fps: u32) -> Result<(), Box<dyn std::error::Error>>;
    fn get_supported_resolutions(&self) -> Result<Vec<Resolution>, Box<dyn std::error::Error>>;
    fn get_supported_formats(
        &self,
        res: Resolution,
    ) -> Result<Vec<DeviceFormat>, Box<dyn std::error::Error>>;
    fn get_supported_framerate(
        &self,
        res: Resolution,
    ) -> Result<Vec<u32>, Box<dyn std::error::Error>>;
    fn get_camera_format(&self) -> DeviceFormat;
    fn set_camera_foramt(&self, format: DeviceFormat);
    fn get_camera_type(&self) -> WebcamType;
    fn open_stream(&mut self) -> Result<StreamType, Box<dyn std::error::Error>>;
}
#[derive(Copy, Clone)]
pub enum WebcamType {
    V4linux2,
    USBVideo,
}
