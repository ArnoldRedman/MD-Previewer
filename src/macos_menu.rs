// macOS 原生菜单栏
use crate::settings::ThemeChoice;
use crate::UserEvent;
use tao::event_loop::EventLoopProxy;

#[cfg(target_os = "macos")]
const GITHUB_URL: &str = "https://github.com/ArnoldRedman/md-preview";

#[cfg(target_os = "macos")]
thread_local! {
    static MACOS_MENU_PROXY: std::cell::RefCell<Option<EventLoopProxy<UserEvent>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_os = "macos")]
fn send_macos_menu_event(event: UserEvent) {
    MACOS_MENU_PROXY.with(|cell| {
        if let Some(proxy) = cell.borrow().as_ref() {
            let _ = proxy.send_event(event);
        }
    });
}

#[cfg(target_os = "macos")]
fn macos_menu_controller_class() -> &'static objc2::runtime::AnyClass {
    use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, NSObject, Sel};
    use objc2::{sel, ClassType, MainThreadOnly};
    use objc2_app_kit::{
        NSAlert, NSAlertStyle, NSButton, NSControlStateValueOff, NSControlStateValueOn, NSImage,
        NSMenuItem, NSView,
    };
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};
    use std::sync::Once;

    extern "C" fn open_file(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::OpenFile);
    }

    extern "C" fn new_file(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::NewFile);
    }

    extern "C" fn close_tab(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::CloseActiveTab);
    }

    extern "C" fn show_find(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::ShowFind);
    }

    extern "C" fn toggle_edit(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::ToggleEdit);
    }

    extern "C" fn print(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::Print);
    }

    extern "C" fn quit(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::Quit);
    }

    extern "C" fn open_github(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::OpenUrl(GITHUB_URL));
    }

    extern "C" fn set_theme(_: &AnyObject, _: Sel, sender: &NSMenuItem) {
        let choice = match sender.tag() {
            102 => ThemeChoice::Light,
            103 => ThemeChoice::Dark,
            _ => ThemeChoice::System,
        };
        let selected = sender.tag();
        if let Some(menu) = unsafe { sender.menu() } {
            for index in 0..menu.numberOfItems() {
                if let Some(item) = menu.itemAtIndex(index) {
                    let tag = item.tag();
                    if (101..=103).contains(&tag) {
                        item.setState(if tag == selected {
                            NSControlStateValueOn
                        } else {
                            NSControlStateValueOff
                        });
                    }
                }
            }
        }
        send_macos_menu_event(UserEvent::SetTheme(choice));
    }

    pub(crate) fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
    }

    pub(crate) fn symbol_button(
        symbol: &str,
        fallback_title: &str,
        tooltip: &str,
        action: Sel,
        target: &AnyObject,
        mtm: MainThreadMarker,
    ) -> objc2::rc::Retained<NSButton> {
        let accessibility = NSString::from_str(tooltip);
        let symbol_name = NSString::from_str(symbol);
        let button = if let Some(image) =
            NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &symbol_name,
                Some(&accessibility),
            ) {
            unsafe {
                NSButton::buttonWithImage_target_action(&image, Some(target), Some(action), mtm)
            }
        } else {
            unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str(fallback_title),
                    Some(target),
                    Some(action),
                    mtm,
                )
            }
        };
        button.setBordered(false);
        button.setToolTip(Some(&accessibility));
        button
    }

    extern "C" fn show_about(controller: &AnyObject, _: Sel, _: &AnyObject) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };

        let alert = NSAlert::new(mtm);
        alert.setAlertStyle(NSAlertStyle::Informational);
        alert.setMessageText(&NSString::from_str("MD Previewer"));
        alert.setInformativeText(&NSString::from_str(&format!(
            "Version {}\n\nFollow local document links between lightweight tabs, keep your reading position between preview and source, and inspect character counts or zoom the content without changing the app chrome.",
            env!("CARGO_PKG_VERSION")
        )));

        let accessory = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, 34.0, 28.0));
        let github = symbol_button(
            "chevron.left.forwardslash.chevron.right",
            "GitHub",
            "GitHub",
            sel!(mdPreviewerOpenGitHub:),
            controller,
            mtm,
        );
        github.setFrame(rect(4.0, 1.0, 26.0, 26.0));
        accessory.addSubview(&github);
        alert.setAccessoryView(Some(&accessory));
        alert.addButtonWithTitle(&NSString::from_str("OK"));

        alert.runModal();
    }

    static REGISTER_CLASS: Once = Once::new();
    REGISTER_CLASS.call_once(|| {
        let mut builder =
            ClassBuilder::new(c"MDPreviewerMenuController", NSObject::class()).unwrap();
        unsafe {
            builder.add_method(
                sel!(mdPreviewerOpenFile:),
                open_file as extern "C" fn(_, _, _),
            );
            builder.add_method(
                sel!(mdPreviewerNewFile:),
                new_file as extern "C" fn(_, _, _),
            );
            builder.add_method(
                sel!(mdPreviewerCloseTab:),
                close_tab as extern "C" fn(_, _, _),
            );
            builder.add_method(
                sel!(mdPreviewerShowFind:),
                show_find as extern "C" fn(_, _, _),
            );
            builder.add_method(
                sel!(mdPreviewerToggleEdit:),
                toggle_edit as extern "C" fn(_, _, _),
            );
            builder.add_method(sel!(mdPreviewerPrint:), print as extern "C" fn(_, _, _));
            builder.add_method(sel!(mdPreviewerQuit:), quit as extern "C" fn(_, _, _));
            builder.add_method(
                sel!(mdPreviewerOpenGitHub:),
                open_github as extern "C" fn(_, _, _),
            );
            builder.add_method(
                sel!(mdPreviewerSetTheme:),
                set_theme as extern "C" fn(_, _, _),
            );
            builder.add_method(
                sel!(mdPreviewerShowAbout:),
                show_about as extern "C" fn(_, _, _),
            );
        }
        let _ = builder.register();
    });

    AnyClass::get(c"MDPreviewerMenuController").unwrap()
}

#[cfg(target_os = "macos")]
pub(crate) fn install_macos_menu(proxy: EventLoopProxy<UserEvent>, theme: ThemeChoice) {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Sel};
    use objc2::{msg_send, sel, MainThreadOnly};
    use objc2_app_kit::{
        NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSEventModifierFlags, NSMenu,
        NSMenuItem,
    };
    use objc2_foundation::{MainThreadMarker, NSString};

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    MACOS_MENU_PROXY.with(|cell| {
        *cell.borrow_mut() = Some(proxy);
    });

    pub(crate) fn menu(title: &str, mtm: MainThreadMarker) -> objc2::rc::Retained<NSMenu> {
        NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title))
    }

    pub(crate) fn item(
        title: &str,
        action: Option<Sel>,
        key: &str,
        modifiers: NSEventModifierFlags,
        mtm: MainThreadMarker,
    ) -> objc2::rc::Retained<NSMenuItem> {
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                action,
                &NSString::from_str(key),
            )
        };
        item.setKeyEquivalentModifierMask(modifiers);
        item
    }

    pub(crate) fn command_item(
        title: &str,
        action: Sel,
        key: &str,
        modifiers: NSEventModifierFlags,
        target: &AnyObject,
        mtm: MainThreadMarker,
    ) -> objc2::rc::Retained<NSMenuItem> {
        let item = item(title, Some(action), key, modifiers, mtm);
        unsafe {
            item.setTarget(Some(target));
        }
        item
    }

    let app = NSApplication::sharedApplication(mtm);
    let main_menu = menu("", mtm);
    let controller: Retained<AnyObject> = unsafe { msg_send![macos_menu_controller_class(), new] };
    let controller_ptr = Retained::into_raw(controller);
    let controller = unsafe { &*controller_ptr };

    let app_menu = menu("MD Previewer", mtm);
    app_menu.setAutoenablesItems(false);
    app_menu.addItem(&command_item(
        "About MD Previewer",
        sel!(mdPreviewerShowAbout:),
        "",
        NSEventModifierFlags::empty(),
        controller,
        mtm,
    ));
    app_menu.addItem(&NSMenuItem::separatorItem(mtm));
    app_menu.addItem(&command_item(
        "GitHub Repository",
        sel!(mdPreviewerOpenGitHub:),
        "",
        NSEventModifierFlags::empty(),
        controller,
        mtm,
    ));
    app_menu.addItem(&NSMenuItem::separatorItem(mtm));
    app_menu.addItem(&command_item(
        "Quit MD Previewer",
        sel!(mdPreviewerQuit:),
        "q",
        NSEventModifierFlags::Command,
        controller,
        mtm,
    ));
    let app_menu_item = item("MD Previewer", None, "", NSEventModifierFlags::empty(), mtm);
    app_menu_item.setSubmenu(Some(&app_menu));
    main_menu.addItem(&app_menu_item);

    let file_menu = menu("File", mtm);
    file_menu.setAutoenablesItems(false);
    file_menu.addItem(&command_item(
        "New Markdown...",
        sel!(mdPreviewerNewFile:),
        "n",
        NSEventModifierFlags::Command,
        controller,
        mtm,
    ));
    file_menu.addItem(&command_item(
        "Open...",
        sel!(mdPreviewerOpenFile:),
        "o",
        NSEventModifierFlags::Command,
        controller,
        mtm,
    ));
    file_menu.addItem(&command_item(
        "Close Tab",
        sel!(mdPreviewerCloseTab:),
        "w",
        NSEventModifierFlags::Command,
        controller,
        mtm,
    ));
    file_menu.addItem(&NSMenuItem::separatorItem(mtm));
    file_menu.addItem(&command_item(
        "Print...",
        sel!(mdPreviewerPrint:),
        "p",
        NSEventModifierFlags::Command,
        controller,
        mtm,
    ));
    let file_menu_item = item("File", None, "", NSEventModifierFlags::empty(), mtm);
    file_menu_item.setSubmenu(Some(&file_menu));
    main_menu.addItem(&file_menu_item);

    let edit_menu = menu("Edit", mtm);
    edit_menu.addItem(&item(
        "Undo",
        Some(sel!(undo:)),
        "z",
        NSEventModifierFlags::Command,
        mtm,
    ));
    edit_menu.addItem(&item(
        "Redo",
        Some(sel!(redo:)),
        "z",
        NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
        mtm,
    ));
    edit_menu.addItem(&NSMenuItem::separatorItem(mtm));
    edit_menu.addItem(&item(
        "Cut",
        Some(sel!(cut:)),
        "x",
        NSEventModifierFlags::Command,
        mtm,
    ));
    edit_menu.addItem(&item(
        "Copy",
        Some(sel!(copy:)),
        "c",
        NSEventModifierFlags::Command,
        mtm,
    ));
    edit_menu.addItem(&item(
        "Paste",
        Some(sel!(paste:)),
        "v",
        NSEventModifierFlags::Command,
        mtm,
    ));
    edit_menu.addItem(&item(
        "Select All",
        Some(sel!(selectAll:)),
        "a",
        NSEventModifierFlags::Command,
        mtm,
    ));
    let edit_menu_item = item("Edit", None, "", NSEventModifierFlags::empty(), mtm);
    edit_menu_item.setSubmenu(Some(&edit_menu));
    main_menu.addItem(&edit_menu_item);

    let view_menu = menu("View", mtm);
    view_menu.setAutoenablesItems(false);
    view_menu.addItem(&command_item(
        "Find",
        sel!(mdPreviewerShowFind:),
        "",
        NSEventModifierFlags::empty(),
        controller,
        mtm,
    ));
    view_menu.addItem(&command_item(
        "Toggle Edit Mode",
        sel!(mdPreviewerToggleEdit:),
        "e",
        NSEventModifierFlags::Command,
        controller,
        mtm,
    ));
    view_menu.addItem(&NSMenuItem::separatorItem(mtm));
    let theme_menu = menu("Theme", mtm);
    theme_menu.setAutoenablesItems(false);
    for (label, choice, tag) in [
        ("System", ThemeChoice::System, 101),
        ("Light", ThemeChoice::Light, 102),
        ("Dark", ThemeChoice::Dark, 103),
    ] {
        let theme_item = command_item(
            label,
            sel!(mdPreviewerSetTheme:),
            "",
            NSEventModifierFlags::empty(),
            controller,
            mtm,
        );
        theme_item.setTag(tag);
        theme_item.setState(if choice == theme {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        theme_menu.addItem(&theme_item);
    }
    let theme_menu_item = item("Theme", None, "", NSEventModifierFlags::empty(), mtm);
    theme_menu_item.setSubmenu(Some(&theme_menu));
    view_menu.addItem(&theme_menu_item);
    let view_menu_item = item("View", None, "", NSEventModifierFlags::empty(), mtm);
    view_menu_item.setSubmenu(Some(&view_menu));
    main_menu.addItem(&view_menu_item);

    app.setMainMenu(Some(&main_menu));
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn install_macos_menu(_proxy: EventLoopProxy<UserEvent>, _theme: ThemeChoice) {}
