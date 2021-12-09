/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

#![no_std]
#![cfg_attr(feature = "mcu-pico-st7789", no_main)]

extern crate alloc;

use alloc::vec::Vec;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use alloc::rc::Rc;
use sixtyfps::Model;

sixtyfps::include_modules!();

/// Returns the current time formated as a string
fn current_time() -> sixtyfps::SharedString {
    #[cfg(feature = "chrono")]
    return chrono::Local::now().format("%H:%M:%S %d/%m/%Y").to_string().into();
    #[cfg(not(feature = "chrono"))]
    return "".into();
}

struct PrinterQueueData {
    data: Rc<sixtyfps::VecModel<PrinterQueueItem>>,
    print_progress_timer: sixtyfps::Timer,
}

impl PrinterQueueData {
    fn push_job(&self, title: sixtyfps::SharedString) {
        self.data.push(PrinterQueueItem {
            status: "WAITING...".into(),
            progress: 0,
            title,
            owner: env!("CARGO_PKG_AUTHORS").into(),
            pages: 1,
            size: "100kB".into(),
            submission_date: current_time(),
        })
    }
}

#[cfg(not(feature = "mcu-pico-st7789"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    #[cfg(feature = "sixtyfps-rendering-backend-mcu")]
    {
        #[cfg(feature = "mcu-simulator")]
        sixtyfps_rendering_backend_mcu::init_simulator();
        #[cfg(not(feature = "mcu-simulator"))]
        sixtyfps_rendering_backend_mcu::init_with_mock_display();
    }

    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    printerdemo_main();
}

#[cfg(feature = "mcu-pico-st7789")]
#[sixtyfps_rendering_backend_mcu::entry]
fn main() -> ! {
    sixtyfps_rendering_backend_mcu::init_board(sixtyfps_rendering_backend_mcu::BoardConfig {
        heap_size: 120 * 1024,
    });
    printerdemo_main();
    loop {}
}

fn printerdemo_main() {
    let main_window = MainWindow::new();
    main_window.set_ink_levels(sixtyfps::VecModel::from_slice(&[
        InkLevel { color: sixtyfps::Color::from_rgb_u8(0, 255, 255), level: 0.40 },
        InkLevel { color: sixtyfps::Color::from_rgb_u8(255, 0, 255), level: 0.20 },
        InkLevel { color: sixtyfps::Color::from_rgb_u8(255, 255, 0), level: 0.50 },
        InkLevel { color: sixtyfps::Color::from_rgb_u8(0, 0, 0), level: 0.80 },
    ]));

    let default_queue: Vec<PrinterQueueItem> =
        main_window.global::<PrinterQueue>().get_printer_queue().iter().collect();
    let printer_queue = Rc::new(PrinterQueueData {
        data: Rc::new(sixtyfps::VecModel::from(default_queue)),
        print_progress_timer: Default::default(),
    });
    main_window
        .global::<PrinterQueue>()
        .set_printer_queue(sixtyfps::ModelHandle::new(printer_queue.data.clone()));

    main_window.on_quit(move || {
        #[cfg(not(target_arch = "wasm32"))]
        sixtyfps::quit_event_loop();
    });

    let printer_queue_copy = printer_queue.clone();
    main_window.global::<PrinterQueue>().on_start_job(move |title| {
        printer_queue_copy.push_job(title);
    });

    let printer_queue_copy = printer_queue.clone();
    main_window.global::<PrinterQueue>().on_cancel_job(move |idx| {
        printer_queue_copy.data.remove(idx as usize);
    });

    let printer_queue_weak = Rc::downgrade(&printer_queue);
    printer_queue.print_progress_timer.start(
        sixtyfps::TimerMode::Repeated,
        core::time::Duration::from_secs(1),
        move || {
            if let Some(printer_queue) = printer_queue_weak.upgrade() {
                if printer_queue.data.row_count() > 0 {
                    let mut top_item = printer_queue.data.row_data(0);
                    top_item.progress += 1;
                    top_item.status = "PRINTING".into();
                    if top_item.progress > 100 {
                        printer_queue.data.remove(0);
                        if printer_queue.data.row_count() == 0 {
                            return;
                        }
                        top_item = printer_queue.data.row_data(0);
                    }
                    printer_queue.data.set_row_data(0, top_item);
                } else {
                    // FIXME: stop this timer?
                }
            }
        },
    );

    main_window.run();
}
