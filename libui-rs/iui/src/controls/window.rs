//! Functionality related to creating, managing, and destroying GUI windows.

use callback_helpers::{from_void_ptr, to_heap_ptr};
use controls::Control;
use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char};
use std::mem;
use std::os::raw::{c_int, c_void};
use std::path::PathBuf;
use ui::UI;
use ui_sys::{self, uiControl, uiFreeText, uiWindow};

use crate::concurrent::queue_main_unsafe;

thread_local! {
    static WINDOWS: RefCell<Vec<Window>> = RefCell::new(Vec::new())
}

/// A `Window` can either have a menubar or not; this enum represents that decision.\
#[derive(Clone, Copy, Debug)]
pub enum WindowType {
    HasMenubar,
    NoMenubar,
}

define_control! {
    /// Contains a single child control and displays it and its children in a window on the screen.
    rust_type: Window,
    sys_type: uiWindow
}

impl Window {
    /// Create a new window with the given title, width, height, and type.
    /// By default, when a new window is created, it will cause the application to quit when closed.
    /// The user can prevent this by adding a custom `on_closing` behavior.
    pub fn new(_ctx: &UI, title: &str, width: c_int, height: c_int, t: WindowType) -> Window {
        let has_menubar = match t {
            WindowType::HasMenubar => true,
            WindowType::NoMenubar => false,
        };
        let mut window = unsafe {
            let c_string = CString::new(title.as_bytes().to_vec()).unwrap();
            let window = Window::from_raw(ui_sys::uiNewWindow(
                c_string.as_ptr(),
                width,
                height,
                has_menubar as c_int,
            ));

            WINDOWS.with(|windows| windows.borrow_mut().push(window.clone()));

            window
        };

        // Windows, by default, quit the application on closing.
        let ui = _ctx.clone();
        window.on_closing(_ctx, move |_| {
            ui.quit();
        });

        // Windows, by default, draw margins
        window.set_margined(_ctx, true);

        window
    }

    /// Get the current title of the window.
    pub fn title(&self, _ctx: &UI) -> String {
        unsafe {
            CStr::from_ptr(ui_sys::uiWindowTitle(self.uiWindow))
                .to_string_lossy()
                .into_owned()
        }
    }

    /// Get a reference to the current title of the window.
    pub fn title_ref(&self, _ctx: &UI) -> &CStr {
        unsafe { &CStr::from_ptr(ui_sys::uiWindowTitle(self.uiWindow)) }
    }

    /// Set the window's title to the given string.
    pub fn set_title(&mut self, _ctx: &UI, title: &str) {
        unsafe {
            let c_string = CString::new(title.as_bytes().to_vec()).unwrap();
            ui_sys::uiWindowSetTitle(self.uiWindow, c_string.as_ptr())
        }
    }

    /// Set a callback to be run when the window closes.
    ///
    /// This is often used on the main window of an application to quit
    /// the application when the window is closed.
    pub fn on_closing<'ctx, F>(&mut self, _ctx: &'ctx UI, callback: F)
    where
        F: FnMut(&mut Window) + 'static,
    {
        extern "C" fn c_callback<G>(window: *mut uiWindow, data: *mut c_void) -> i32
        where
            G: FnMut(&mut Window),
        {
            let mut window = Window { uiWindow: window };
            unsafe {
                from_void_ptr::<G>(data)(&mut window);
            }
            0
        }

        unsafe {
            ui_sys::uiWindowOnClosing(self.uiWindow, Some(c_callback::<F>), to_heap_ptr(callback));
        }
    }

    pub fn set_borderless(&mut self, _ctx: &UI, borderless: bool)
    {
        unsafe {
            ui_sys::uiWindowSetFullscreen(self.uiWindow, borderless as i32);
        }
    }

    pub fn set_fullscreen(&mut self, _ctx: &UI, fullscreen: bool)
    {
        unsafe {
            ui_sys::uiWindowSetFullscreen(self.uiWindow, fullscreen as i32);
        }
    }

    /// Check whether or not this window has margins around the edges.
    pub fn margined(&self, _ctx: &UI) -> bool {
        unsafe { ui_sys::uiWindowMargined(self.uiWindow) != 0 }
    }

    /// Set whether or not the window has margins around the edges.
    pub fn set_margined(&mut self, _ctx: &UI, margined: bool) {
        unsafe { ui_sys::uiWindowSetMargined(self.uiWindow, margined as c_int) }
    }

    /// Sets the window's child widget. The window can only have one child widget at a time.
    pub fn set_child<T: Into<Control>>(&mut self, _ctx: &UI, child: T) {
        unsafe { ui_sys::uiWindowSetChild(self.uiWindow, child.into().as_ui_control()) }
    }

    /// Allow the user to select an existing file using the systems file dialog
    pub fn open_file(&self, _ctx: &UI) -> Option<PathBuf> {
        let ptr = unsafe { ui_sys::uiOpenFile(self.uiWindow) };
        if ptr.is_null() {
            return None;
        };
        let path_string: String = unsafe { CStr::from_ptr(ptr).to_string_lossy().into() };
        unsafe {
            uiFreeText(ptr);
        }
        Some(path_string.into())
    }

    /// Allow the user to select a new or existing file using the systems file dialog.
    pub fn save_file(&self, _ctx: &UI) -> Option<PathBuf> {
        let ptr = unsafe { ui_sys::uiSaveFile(self.uiWindow) };
        if ptr.is_null() {
            return None;
        };
        let path_string: String = unsafe { CStr::from_ptr(ptr).to_string_lossy().into() };
        unsafe {
            uiFreeText(ptr);
        }
        Some(path_string.into())
    }

    pub fn save_file_with_filter(&self, _ctx: &UI, filters: &[FileTypeFilter]) -> Option<PathBuf> {
        let filters: Vec<ui_sys::uiFileTypeFilter> = filters
            .iter()
            .map(|f| unsafe { f.as_ui_file_type_filter() })
            .collect();
        let ptr = unsafe { ui_sys::uiSaveFile2(self.uiWindow, filters.as_ptr(), filters.len() as c_int) };
        if ptr.is_null() {
            return None;
        };
        let path_string: String = unsafe { CStr::from_ptr(ptr).to_string_lossy().into() };
        unsafe {
            uiFreeText(ptr);
        }
        Some(path_string.into())
    }

    /// Allow the user to select a single folder using the systems folder dialog.
    pub fn open_folder(&self, _ctx: &UI) -> Option<PathBuf> {
        let ptr = unsafe { ui_sys::uiOpenFolder(self.uiWindow) };
        if ptr.is_null() {
            return None;
        };
        let path_string: String = unsafe { CStr::from_ptr(ptr).to_string_lossy().into() };
        unsafe {
            uiFreeText(ptr);
        }
        Some(path_string.into())
    }

    /// Open a generic message box to show a message to the user.
    /// Returns a future that resolves when the user acknowledges the message.
    ///
    /// DO NOT USE IN ASYNC CODE.
    pub fn modal_msg(&self, _ctx: &UI, title: &str, description: &str) {
        unsafe {
            let c_title = CString::new(title.as_bytes().to_vec()).unwrap();
            let c_description = CString::new(description.as_bytes().to_vec()).unwrap();
            ui_sys::uiMsgBox(self.uiWindow, c_title.as_ptr(), c_description.as_ptr())
        }
    }

    pub fn modal_msg_async(&self, _ctx: &UI, title: &str, description: &str) -> impl std::future::Future {
        let c_title = CString::new(title.as_bytes().to_vec()).unwrap();
        let c_description = CString::new(description.as_bytes().to_vec()).unwrap();
        let window = self.uiWindow;
        Modal {
            f: Some(move || unsafe {
                ui_sys::uiMsgBox(window, c_title.as_ptr(), c_description.as_ptr());
            })
        }
    }

    /// Open an error-themed message box to show a message to the user.
    /// Returns a future that resolves when the user acknowledges the message.
    ///
    /// DO NOT USE IN ASYNC CODE.
    pub fn modal_err(&self, _ctx: &UI, title: &str, description: &str) {
        unsafe {
            let c_title = CString::new(title.as_bytes().to_vec()).unwrap();
            let c_description = CString::new(description.as_bytes().to_vec()).unwrap();
            ui_sys::uiMsgBoxError(self.uiWindow, c_title.as_ptr(), c_description.as_ptr())
        }
    }

    pub fn modal_err_async(&self, _ctx: &UI, title: &str, description: &str) -> impl std::future::Future {
        let c_title = CString::new(title.as_bytes().to_vec()).unwrap();
        let c_description = CString::new(description.as_bytes().to_vec()).unwrap();
        let window = self.uiWindow;
        Modal {
            f: Some(move || unsafe {
                ui_sys::uiMsgBoxError(window, c_title.as_ptr(), c_description.as_ptr());
            })
        }
    }

    pub unsafe fn destroy_all_windows() {
        WINDOWS.with(|windows| {
            let mut windows = windows.borrow_mut();
            for window in windows.drain(..) {
                window.destroy();
            }
        })
    }

    /// Destroys a Window. Any use of the control after this is use-after-free; therefore, this
    /// is marked unsafe.
    pub unsafe fn destroy(&self) {
        // Don't check for initialization here since this can be run during deinitialization.
        ui_sys::uiControlDestroy(self.uiWindow as *mut ui_sys::uiControl)
    }
}

/// A future designed to work with the recursive main loop that gtk creates when creating modals.
struct Modal<F> {
    f: Option<F>
}

impl<F: FnOnce() + Unpin + 'static> std::future::Future for Modal<F> {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        if let Some(f) = self.f.take() {
            let waker = cx.waker().clone();
            queue_main_unsafe(move || {
                f();
                waker.wake();
            });
        }
        std::task::Poll::Ready(())
    }
}

pub struct FileTypeFilter {
    name: CString,
    extensions: Vec<*mut c_char>,
}

impl FileTypeFilter {
    pub fn new(name: &str) -> Self {
        let name = CString::new(name.as_bytes().to_vec()).unwrap();
        Self {
            name: name.into(),
            extensions: Vec::new(),
        }
    }
    pub fn extension(mut self, extension: &str) -> Self {
        let e = CString::new(extension.as_bytes().to_vec()).unwrap().into_raw();
        self.extensions.push(e);
        self
    }
    /// The return value borrows from `self`.
    pub unsafe fn as_ui_file_type_filter(&self) -> ui_sys::uiFileTypeFilter {
        ui_sys::uiFileTypeFilter {
            name: self.name.as_ptr() as _,
            extensions: self.extensions.as_ptr() as _,
            extensions_len: self.extensions.len() as i32,
        }
    }
}

impl Drop for FileTypeFilter {
    fn drop(&mut self) {
        for &e in &self.extensions {
            let _ = unsafe { CString::from_raw(e) };
        }
    }
}
