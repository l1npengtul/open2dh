use crate::nodes::editor_tabs::util::create_editable_item;
use gdnative::{
    api::{tree::Tree, tree_item::*},
    prelude::*,
    NativeClass,
    core_types::Rect2,
};
use uvc::Device;

#[derive(NativeClass)]
#[inherit(Tree)]
pub struct WebcamInputEditor;

#[methods]
impl WebcamInputEditor {
    fn new(_owner: &Tree) -> Self {
        WebcamInputEditor
    }
    #[export]
    fn _ready(&self, owner: TRef<Tree>) {
        let root_item: &TreeItem = unsafe {
            &*owner
                .create_item(owner.assume_shared(), 0)
                .unwrap()
                .assume_safe()
        };

        owner.set_hide_root(true);
        owner.set_columns(2);

        let webcam_video_input: &TreeItem = unsafe {
            &*owner
                .create_item(root_item.assume_shared(), 1)
                .unwrap()
                .assume_safe()
        };
        webcam_video_input.set_text(0, "Webcam Input Settings");
        webcam_video_input.set_text_align(0, TreeItem::ALIGN_CENTER);

        let webcam_select_list: &TreeItem = unsafe {
            &*owner.create_item(root_item.assume_shared(), 2)
                .unwrap()
                .assume_safe()
        };
        webcam_select_list.set_text(0, "Input Webcam:");
        webcam_select_list.set_text_align(0, 0);
        webcam_select_list.set_cell_mode(1, 4);
        webcam_select_list.set_editable(1, true);
        webcam_select_list.set_custom_draw(1, owner, "webcam_editor_clicked");

        owner.connect("custom_popup_edited", owner, "item_clicked", VariantArray::new_shared(), 0);
    }

    #[export]
    // The documentation on this is piss poor, so this will probably be wrong. Trial and error the function arguments until it works.
    fn webcam_editor_clicked(&self, owner: TRef<Tree>, treeitem: Ref<TreeItem>, rect: Rect2) {
        let uvc_devices = match crate::UVC.devices() {
            Ok(dev) => {
                godot_print!("b");
                dev
            }
            Err(why) => {
                // show error
                return;
            }
        };
        // Change to directly filling menu
        let mut devlist: Vec<Device> = Vec::new();
        for i in uvc_devices.into_iter() {
            devlist.push(i);
        }
    }
}
