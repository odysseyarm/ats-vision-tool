//! Modified version of `iui::layout!` that supports leptos signals and effects.
//! Changes:
//! * enabled property on Entry

use leptos_reactive::{MaybeSignal, Memo, ReadSignal, RwSignal, Signal};

/// Creates layout code from a compact, declarative and hierarchical UI description.
///
/// # Example
///
/// For a more example, see the [builder example application](https://github.com/iui-rs/libui/tree/development/libui/examples) in the repository.
///
/// ```no_run
/// extern crate iui;
/// use iui::prelude::*;
///
/// fn main() {
///     let ui = UI::init().unwrap();
///
///     iui::layout! { &ui,
///         let layout = VerticalBox(padded: true) {
///             Compact: let form = Form(padded: true) {
///                 (Compact, "User"): let tb_user = Entry()
///                 (Compact, "Password"): let tb_passwd = Entry()
///             }
///             Stretchy: let bt_submit = Button("Submit")
///         }
///     }
///
///     let mut window = Window::new(&ui, "Builder Example", 320, 200,
///         WindowType::NoMenubar);
///
///     window.set_child(&ui, layout);
///     window.show(&ui);
///     ui.main();
/// }
/// ```
#[macro_export]
macro_rules! layout {

    // ---------------------- Controls without children -----------------------
    [ $ui:expr ,
        let $ctl:ident = Area ( $handler:expr )
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::Area::new($ui, $handler);
    ];

    // Button
    [ $ui:expr ,
        let $ctl:ident = Button ( $text:expr $( , enabled: $enabled:expr $(,)? )? )
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::Button::new($ui, "");
        leptos_reactive::create_effect({
            let ui = iui::UI::clone($ui);
            let ctl = $ctl.clone();
            let text = $crate::layout_macro::IntoMaybeSignal::<String>::from($text);
            move |_| {
                let mut ctl = ctl.clone();
                leptos_reactive::SignalWith::with(&text, |text| {
                    ctl.set_text(&ui, &text);
                });
            }
        });
        $(
            leptos_reactive::create_effect({
                let ui = iui::UI::clone($ui);
                let ctl = $ctl.clone();
                let enabled = $crate::layout_macro::IntoMaybeSignal::<bool>::from($enabled);
                move |_| {
                    let mut ctl = ctl.clone();
                    match leptos_reactive::SignalGet::get(&enabled) {
                        true => ctl.enable(&ui),
                        false => ctl.disable(&ui),
                    }
                }
            });
        )?
    ];

    // Checkbox
    [ $ui:expr ,
        let $ctl:ident = Checkbox ( $text:expr $( , checked: $checked:expr )? )
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::Checkbox::new($ui, $text);
        $( $ctl.set_checked($ui, $checked); )?
    ];

    // ColorButton
    [ $ui:expr ,
        let $ctl:ident = ColorButton ()
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::ColorButton::new();
    ];

    // Combobox
    [ $ui:expr ,
        let $ctl:ident = Combobox ( $( $prop:ident: $value:expr ),* $(,)? )
        { $( $option:expr ),* }
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::Combobox::new($ui);
        $( $ctl.append($ui, $option); )*
        $($crate::layout!(@args Combobox $ui; $ctl $prop $value);)*
    ];

    // DateTimePicker
    [ $ui:expr ,
        let $ctl:ident = DateTimePicker ( $kind:ident )
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::DateTimePicker::new(
            iui::controls::DateTimePickerKind::$kind);
    ];

    // EditableCombobox
    [ $ui:expr ,
        let $ctl:ident = EditableCombobox ( $( $prop:ident: $value:expr ),* $(,)? )
        { $( $option:expr ),* }
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::EditableCombobox::new($ui);
        $( $ctl.append($ui, $option); )*
        $($crate::layout!(@args EditableCombobox $ui; $ctl $prop $value);)*
    ];

    // Entry
    [ $ui:expr ,
        let $ctl:ident = Entry ( $( $prop:ident: $value:expr ),* $(,)? )
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::Entry::new($ui);
        $($crate::layout!(@args Entry $ui; $ctl $prop $value);)*
    ];

    // FontButton
    [ $ui:expr ,
        let $ctl:ident = FontButton ()
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::FontButton::new($ui);
    ];

    // HorizontalSeparator
    [ $ui:expr ,
        let $ctl:ident = HorizontalSeparator ()
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::HorizontalSeparator::new($ui);
    ];

    // Label
    [ $ui:expr ,
        let $ctl:ident = Label ( $text:literal )
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::Label::new($ui, $text);
    ];
    [ $ui:expr ,
        let $ctl:ident = Label ( $text:expr )
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::Label::new($ui, "");
        leptos_reactive::create_effect({
            let ui = iui::UI::clone($ui);
            let ctl = $ctl.clone();
            let text = $crate::layout_macro::IntoMaybeSignal::<String>::from($text);
            move |_| {
                let mut ctl = ctl.clone();
                leptos_reactive::SignalWith::with(&text, |text| {
                    ctl.set_text(&ui, &text);
                });
            }
        });
    ];

    // MultilineEntry
    [ $ui:expr ,
        let $ctl:ident = MultilineEntry ()
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::MultilineEntry::new($ui);
    ];

    // MultilineEntry, wrapping option
    [ $ui:expr ,
        let $ctl:ident = MultilineEntry ( wrapping: $wrapping:expr )
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = if $wrapping {
            iui::controls::MultilineEntry::new($ui)
        } else {
            iui::controls::MultilineEntry::new_nonwrapping($ui)
        };
    ];

    // PasswordEntry
    [ $ui:expr ,
        let $ctl:ident = PasswordEntry ()
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::PasswordEntry::new($ui);
    ];

    // RadioButtons
    [ $ui:expr ,
        let $ctl:ident = RadioButtons ( $( selected: $selected:expr )? )
        { $( $option:expr ),* }
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::RadioButtons::new($ui);
        $( $ctl.append($option); )*
        $( $ctl.set_selected($selected); )?
    ];

    // SearchEntry
    [ $ui:expr ,
        let $ctl:ident = SearchEntry ()
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::SearchEntry::new($ui);
    ];

    // Slider
    [ $ui:expr ,
        let $ctl:ident = Slider ( $min:expr , $max:expr )
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::Slider::new($ui, $min, $max);
    ];

    // Spacer
    [ $ui:expr ,
        let $ctl:ident = Spacer ()
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::Spacer::new($ui);
    ];

    // Spinbox, limited
    [ $ui:expr ,
        let $ctl:ident = Spinbox ( $min:expr , $max:expr $(, $prop:ident: $value:expr )* $(,)?)
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::Spinbox::new($ui, $min, $max);
        $($crate::layout!(@args Spinbox $ui; $ctl $prop $value);)*
    ];

    // Spinbox, unlimited
    [ $ui:expr ,
        let $ctl:ident = Spinbox ( $( $prop:ident: $value:expr ),* $(,)? )
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::Spinbox::new_unlimited($ui);
        $($crate::layout!(@args Spinbox $ui; $ctl $prop $value);)*
    ];

    // ProgressBar
    [ $ui:expr ,
        let $ctl:ident = ProgressBar ( $($value:expr )? )
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::ProgressBar::new($ui);
        $(
            leptos_reactive::create_effect({
                let ui = iui::UI::clone($ui);
                let ctl = $ctl.clone();
                let value = $crate::layout_macro::IntoMaybeSignal::<u32>::from($value);
                move |_| {
                    let mut ctl = ctl.clone();
                    ctl.set_value(&ui, leptos_reactive::SignalGet::get(&value));
                }
            });
        )?
    ];

    // ----------------- Controls with children (Containers) ------------------

    // Form
    [ $ui:expr ,
        let $ctl:ident = Form ( $( padded: $padded:expr )? )
        { $(
            ( $strategy:ident, $name:expr ) :
            let $child:ident = $type:ident ($($opt:tt)*) $({$($body:tt)*})?
        )* }
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::Form::new($ui);
        $( $ctl.set_padded($ui, $padded); )?
        $(
            $crate::layout! { $ui, let $child = $type ($($opt)*) $({$($body)*})? }
            $ctl.append($ui, $name, $child.clone(),
                    iui::controls::LayoutStrategy::$strategy);
        )*
    ];

    // Group
    [ $ui:expr ,
        let $ctl:ident = Group ( $title:expr $( , margined: $margined:expr )? )
        { $(
            let $child:ident = $type:ident ($($opt:tt)*) $({$($body:tt)*})?
        )? }
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::Group::new($title);
        $( $ctl.set_margined($margined); )?
        $(
            $crate::layout! { $ui, let $child = $type ($($opt)*) $({$($body)*})? }
            $ctl.set_child($child.clone());
        )?
    ];

    // HorizontalBox
    [ $ui:expr ,
        let $ctl:ident = HorizontalBox ( $( padded: $padded:expr )? )
        { $(
            $strategy:ident :
            let $child:ident = $type:ident ($($opt:tt)*) $({$($body:tt)*})?
        )* }
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::HorizontalBox::new($ui);
        $( $ctl.set_padded($ui, $padded); )?
        $(
            $crate::layout! { $ui, let $child = $type ($($opt)*) $({$($body)*})? }
            $ctl.append($ui, $child.clone(),
                        iui::controls::LayoutStrategy::$strategy);
        )*
    ];

    // LayoutGrid
    [ $ui:expr ,
        let $ctl:ident = LayoutGrid ( $( padded: $padded:expr )? )
        { $(
            $( #[$attr:meta] )*
            ( $x:expr , $y:expr ) ( $xspan:expr , $yspan:expr )
            $expand:ident ( $halign:ident , $valign:ident ) :
            let $child:ident = $type:ident ($($opt:tt)*) $({$($body:tt)*})?
        )* }
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::LayoutGrid::new($ui);
        $( $ctl.set_padded($ui, $padded); )?
        $(
            $( #[$attr] )*
            $crate::layout! { $ui, let $child = $type ($($opt)*) $({$($body)*})? }
            $( #[$attr] )*
            $ctl.append($ui, $child.clone(), $x, $y, $xspan, $yspan,
                        iui::controls::GridExpand::$expand,
                        iui::controls::GridAlignment::$halign,
                        iui::controls::GridAlignment::$valign);
        )*
    ];

    // TabGroup
    [ $ui:expr ,
        let $ctl:ident = TabGroup ()
        { $(
            ( $name:expr $( , margined: $margined:expr )? ) :
            let $child:ident = $type:ident ($($opt:tt)*) $({$($body:tt)*})?
        )* }
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::TabGroup::new($ui);
        $(
            $crate::layout! { $ui, let $child = $type ($($opt)*) $({$($body)*})? }
            let __tab_n = $ctl.append($ui, $name, $child.clone());
            $( $ctl.set_margined($ui, __tab_n - 1, $margined); )?
        )*
    ];

    // VerticalBox
    [ $ui:expr ,
        let $ctl:ident = VerticalBox ( $( padded: $padded:expr )? )
        { $(
            $strategy:ident :
            let $child:ident = $type:ident ($($opt:tt)*) $({$($body:tt)*})?
        )* }
    ] => [
        #[allow(unused_mut)]
        let mut $ctl = iui::controls::VerticalBox::new($ui);
        $( $ctl.set_padded($ui, $padded); )?
        $(
            $crate::layout! { $ui, let $child = $type ($($opt)*) $({$($body)*})? }
            $ctl.append($ui, $child.clone(),
                        iui::controls::LayoutStrategy::$strategy);
        )*
    ];

    // Control properties
    [ @args Spinbox $ui:expr; $ctl:ident value $value:expr] => [
        leptos_reactive::create_effect({
            let ui = iui::UI::clone($ui);
            let ctl = $ctl.clone();
            let value = $crate::layout_macro::IntoMaybeSignal::from($value);
            move |_| {
                let mut ctl = ctl.clone();
                leptos_reactive::SignalWith::with(&value, |v| iui::controls::NumericEntry::set_value(&mut ctl, &ui, *v));
            }
        })
    ];
    [ @args $ty:ident $ui:expr; $ctl:ident value $value:expr] => [
        leptos_reactive::create_effect({
            let ui = iui::UI::clone($ui);
            let ctl = $ctl.clone();
            let value = $crate::layout_macro::IntoMaybeSignal::<String>::from($value);
            move |_| {
                let mut ctl = ctl.clone();
                leptos_reactive::SignalWith::with(&value, |v| iui::controls::TextEntry::set_value(&mut ctl, &ui, v));
            }
        })
    ];
    [ @args $ty:ident $ui:expr; $ctl:ident enabled $enabled:expr] => [
        leptos_reactive::create_effect({
            let ui = iui::UI::clone($ui);
            let ctl = $ctl.clone();
            let enabled = $crate::layout_macro::IntoMaybeSignal::from($enabled);
            move |_| {
                let mut ctl = ctl.clone();
                if leptos_reactive::SignalGet::get(&enabled) {
                    ctl.enable(&ui);
                } else {
                    ctl.disable(&ui);
                }
            }
        });
    ];
    [ @args Spinbox $ui:expr; $ctl:ident onchange $signal:expr] => [
        iui::controls::NumericEntry::on_changed(&mut $ctl, $ui, move |v| leptos_reactive::SignalSet::set(&$signal, v))
    ];
    [ @args $ty:ident $ui:expr; $ctl:ident onchange $signal:expr] => [
        iui::controls::TextEntry::on_changed(&mut $ctl, $ui, move |v| leptos_reactive::SignalSet::set(&$signal, v))
    ];
    [ @args Spinbox $ui:expr; $ctl:ident signal $signal:expr] => [
        $crate::layout!(@args Spinbox $ui; $ctl value $signal);
        $crate::layout!(@args Spinbox $ui; $ctl onchange $signal);
    ];
    [ @args Entry $ui:expr; $ctl:ident signal $signal:expr] => [
        $crate::layout!(@args Entry $ui; $ctl value $signal);
        $crate::layout!(@args Entry $ui; $ctl onchange $signal);
    ];
    [ @args $ty:ident $ui:expr; $ctl:ident selected $selected:expr] => [
        leptos_reactive::create_effect({
            let ui = iui::UI::clone($ui);
            let ctl = $ctl.clone();
            let value = $crate::layout_macro::IntoMaybeSignal::from($selected);
            move |_| {
                let mut ctl = ctl.clone();
                ctl.set_selected(&ui, leptos_reactive::SignalGet::get(&value));
            }
        })
    ];
    [ @args $ty:ident $ui:expr; $ctl:ident onselect $signal:expr] => [
        $ctl.on_selected($ui, move |v| leptos_reactive::SignalSet::set(&$signal, v))
    ];
    [ @args Combobox $ui:expr; $ctl:ident signal $signal:expr] => [
        $crate::layout!(@args Combobox $ui; $ctl selected $signal);
        $crate::layout!(@args Combobox $ui; $ctl onselect $signal);
    ];
}

/// An alternate set of impls of From<...> for MaybeSignal<T>
pub trait IntoMaybeSignal<T> {
    fn from(self) -> MaybeSignal<T>;
}

impl IntoMaybeSignal<bool> for bool {
    fn from(self) -> MaybeSignal<bool> {
        MaybeSignal::Static(self)
    }
}

impl IntoMaybeSignal<i32> for i32 {
    fn from(self) -> MaybeSignal<i32> {
        MaybeSignal::Static(self)
    }
}

impl IntoMaybeSignal<u32> for u32 {
    fn from(self) -> MaybeSignal<u32> {
        MaybeSignal::Static(self)
    }
}

impl<T> IntoMaybeSignal<T> for ReadSignal<T> {
    fn from(self) -> MaybeSignal<T> {
        MaybeSignal::Dynamic(self.into())
    }
}

impl<T> IntoMaybeSignal<T> for RwSignal<T> {
    fn from(self) -> MaybeSignal<T> {
        MaybeSignal::Dynamic(self.into())
    }
}

impl<T> IntoMaybeSignal<T> for Memo<T> {
    fn from(self) -> MaybeSignal<T> {
        MaybeSignal::Dynamic(self.into())
    }
}

impl<T> IntoMaybeSignal<T> for Signal<T> {
    fn from(self) -> MaybeSignal<T> {
        MaybeSignal::Dynamic(self)
    }
}

impl IntoMaybeSignal<String> for &str {
    fn from(self) -> MaybeSignal<String> {
        MaybeSignal::Static(self.to_string())
    }
}

impl<F, T, U> IntoMaybeSignal<T> for F
where
    F: Fn() -> U + 'static,
    U: Into<T>,
{
    fn from(self) -> MaybeSignal<T> {
        MaybeSignal::derive(move || self().into())
    }
}
