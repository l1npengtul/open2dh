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

use crate::show_error;
use dirs::home_dir;
use gdnative::{
    api::{FileDialog, MenuButton, PopupMenu, OS},
    methods,
    prelude::*,
    NativeClass,
};
use native_dialog::{Error, FileDialog as NativeFileDialog, Filter, MessageDialog};
use std::path::Path;
use std::{cell::RefCell, ffi::OsString, path::PathBuf};

#[derive(NativeClass)]
#[inherit(MenuButton)]
pub struct FileMenuButton {
    previous_file_path: RefCell<String>,
}

// TODO: signal to connect to Viewport and change model
#[methods]
impl FileMenuButton {
    fn new(_owner: &MenuButton) -> Self {
        let home_dir = match home_dir() {
            Some(h) => match h.into_os_string().into_string() {
                Ok(p) => p,
                Err(_) => {
                    let os = OS::godot_singleton();
                    os.get_user_data_dir().to_string()
                }
            },
            None => {
                let os = OS::godot_singleton();
                os.get_user_data_dir().to_string()
            }
        };
        FileMenuButton {
            previous_file_path: RefCell::new(home_dir),
        }
    }
    #[export]
    fn _ready(&self, owner: TRef<MenuButton>) {
        let popupmenu = unsafe { &*owner.get_popup().unwrap().assume_safe() };
        popupmenu.add_item("Open Model", 0, -1);
        popupmenu.connect(
            "id_pressed",
            self,
            "on_popupmenu_button_clicked",
            VariantArray::new_shared(),
            0,
        );
    }

    #[export]
    pub fn on_popupmenu_button_clicked(&self, owner: TRef<MenuButton>, id: i32) {
        match id {
            0 => {
                match NativeFileDialog::new()
                    .set_location(&*self.previous_file_path.borrow())
                    .add_filter("glTF Model", &["*.gltf", "*.glb"])
                    .add_filter("VRM Model", &["*.vrm"])
                    .add_filter("FBX Model", &["*.fbx"])
                    .add_filter("Collada Model", &["*.dae"])
                    .show_open_single_file()
                {
                    Ok(path) => {
                        match path {
                            Some(p) => {
                                match p.parent() {
                                    Some(dir_path) => {
                                        let path_str = dir_path
                                            .as_os_str()
                                            .into_os_string()
                                            .into_string()
                                            .unwrap();
                                        *self.previous_file_path.borrow_mut() = path_str
                                    }
                                    None => {
                                        let path_str = p.into_os_string().into_string().unwrap();
                                        *self.previous_file_path.borrow_mut() = path_str
                                    }
                                }
                                // TODO: Loader emit signal
                            }
                            None => {
                                show_error!("Failed to open file", "File path doesn't exist!")
                            }
                        }
                    }
                    Err(why) => {
                        show_error!("Failed to open file", why)
                    }
                }
            }
            _ => {}
        }
    }
}

#[derive(NativeClass)]
#[inherit(MenuButton)]
pub struct EditMenuButton;

#[methods]
impl EditMenuButton {
    fn new(_owner: &MenuButton) -> Self {
        EditMenuButton
    }
    #[export]
    fn _ready(&self, owner: TRef<MenuButton>) {
        let popupmenu = unsafe { &*owner.get_popup().unwrap().assume_safe() };
        popupmenu.add_item("Open Settings", 0, -1);
        popupmenu.connect(
            "id_pressed",
            self,
            "on_popupmenu_button_clicked",
            VariantArray::new_shared(),
            0,
        );
    }

    #[export]
    pub fn on_popupmenu_button_clicked(&self, owner: TRef<MenuButton>, id: i32) {
        match id {
            0 => {}
            _ => {}
        }
    }
}

#[derive(NativeClass)]
#[inherit(MenuButton)]
pub struct HelpMenuButton;

// TODO: signal to connect to Viewport and change model
#[methods]
impl HelpMenuButton {
    fn new(_owner: &MenuButton) -> Self {
        HelpMenuButton
    }
    #[export]
    fn _ready(&self, owner: TRef<MenuButton>) {
        let popupmenu = unsafe { &*owner.get_popup().unwrap().assume_safe() };
        popupmenu.add_item("Open Docs", 0, -1); // TODO: Fix Nonexistant Docs
        popupmenu.add_separator("");
        popupmenu.add_item("About", 1, -1);
        popupmenu.connect(
            "id_pressed",
            self,
            "on_popupmenu_button_clicked",
            VariantArray::new_shared(),
            0,
        );
    }

    #[export]
    pub fn on_popupmenu_button_clicked(&self, owner: TRef<MenuButton>, id: i32) {
        match id {
            0 => {}
            1 => {}
            _ => {}
        }
    }
}
