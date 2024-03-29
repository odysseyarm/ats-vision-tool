use super::Control;
use callback_helpers::{from_void_ptr, to_heap_ptr};
use std::mem;
use std::mem::MaybeUninit;
use std::os::raw::c_void;
use ui::UI;
use ui_sys::{self, uiControl, uiDateTimePicker};

define_control! {
    rust_type: DateTimePicker,
    sys_type: uiDateTimePicker
}

pub enum DateTimePickerKind {
    DateTime,
    Date,
    Time,
}

impl DateTimePicker {
    /// Create a new date and/or time picker.
    pub fn new(_ctx: &UI, mode: DateTimePickerKind) -> DateTimePicker {
        unsafe {
            DateTimePicker::from_raw(match mode {
                DateTimePickerKind::DateTime => ui_sys::uiNewDateTimePicker(),
                DateTimePickerKind::Date => ui_sys::uiNewDatePicker(),
                DateTimePickerKind::Time => ui_sys::uiNewTimePicker(),
            })
        }
    }

    /// Returns date and time stored in the DateTimePicker.
    ///
    /// Warning: The `struct tm` members `tm_wday` and `tm_yday` are undefined
    pub fn datetime(&self, _ctx: &UI) -> ui_sys::tm {
        unsafe {
            let mut datetime = MaybeUninit::<ui_sys::tm>::uninit();
            ui_sys::uiDateTimePickerTime(self.uiDateTimePicker, datetime.as_mut_ptr());
            datetime.assume_init()
        }
    }

    /// Sets date and time of the DateTimePicker.
    ///
    /// Warning: The `struct tm` member `tm_isdst` is ignored on Windows and should be set to `-1`
    pub fn set_datetime(&self, _ctx: &UI, datetime: ui_sys::tm) {
        unsafe {
            let ptr = &datetime as *const ui_sys::tm;
            ui_sys::uiDateTimePickerSetTime(self.uiDateTimePicker, ptr as *const ui_sys::tm);
        }
    }

    /// Registers a callback for when the date time picker value is changed by the user.
    ///
    /// The callback is not triggered when calling set_datetime().
    /// Only one callback can be registered at a time.
    pub fn on_changed<'ctx, F>(&mut self, _ctx: &'ctx UI, callback: F)
    where
        F: FnMut(&mut DateTimePicker) + 'static,
    {
        extern "C" fn c_callback<G>(picker: *mut uiDateTimePicker, data: *mut c_void)
        where
            G: FnMut(&mut DateTimePicker),
        {
            let mut picker = DateTimePicker {
                uiDateTimePicker: picker,
            };
            unsafe {
                from_void_ptr::<G>(data)(&mut picker);
            }
        }
        unsafe {
            ui_sys::uiDateTimePickerOnChanged(
                self.uiDateTimePicker,
                Some(c_callback::<F>),
                to_heap_ptr(callback),
            );
        }
    }
}
