//     Open2DH - Open 2D Holo, a program to procedurally animate your face onto an 3D Model.
//     Copyright (C) 2020-2021 l1npengtul
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
#![feature(maybe_uninit_ref)]
#![feature(maybe_uninit_extra)]
#![deny(clippy::pedantic)]
#![warn(clippy::all)]
// NO MATTER WHAT LINT COMES THROUGH THAT GATE
#![allow(clippy::clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::unused_self)]
#![allow(clippy::match_wild_err_arm)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]

use crate::util::camera::device_utils::DeviceContact;
use gdnative::api::OS;
use gdnative::prelude::*;
use opencv::core::CV_8UC3;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use uvc::Context;

pub mod configuration;
pub mod error;
pub mod nodes;
pub mod processing;
pub mod util;

#[macro_use]
extern crate lazy_static;
extern crate ouroboros;

// Make it so we can get a webcam stream anywhere so we don't have to deal with 'static bullshit
lazy_static! {
    static ref UVC: Arc<Context<'static>> = Arc::new(Context::new().unwrap());
}

thread_local! {
    pub(crate) static CURRENT_DEVICE: Rc<RefCell<Option<DeviceContact>>> = Rc::new(RefCell::new(None));
}

fn init(handle: InitHandle) {
    handle.add_class::<nodes::open2dhctrl::Open2DHCtrl>();
    handle.add_class::<nodes::model_tree_edit::ModelTreeEditor>();
    handle.add_class::<nodes::webcam_input_edit::WebcamInputEditor>();
    handle.add_class::<nodes::viewport_holder::ViewportHolder>();
    handle.add_class::<nodes::upper_tab_popups::FileMenuButton>();
    handle.add_class::<nodes::upper_tab_popups::EditMenuButton>();
    handle.add_class::<nodes::upper_tab_popups::HelpMenuButton>();
    handle.add_class::<nodes::settings_dialog::SettingsDialog>();
    handle.add_class::<nodes::about_dialog::AboutDialog>();
}

godot_init!(init);
