#![no_std]
#![feature(alloc)]
#![feature(asm)]
#![feature(compiler_builtins_lib)]
#![feature(const_fn)]
#![feature(lang_items)]

#[macro_use]
extern crate alloc;
extern crate compiler_builtins;
extern crate ecflash;
extern crate orbclient;
extern crate uefi;
extern crate uefi_alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::char;
use ecflash::{Ec, EcFile, EcFlash};
use orbclient::{Color, Renderer};

use display::{Display, Output};
use proto::Protocol;

pub static mut HANDLE: uefi::Handle = uefi::Handle(0);
pub static mut UEFI: *mut uefi::system::SystemTable = 0 as *mut uefi::system::SystemTable;

#[macro_use]
mod macros;

pub mod console;
pub mod display;
pub mod externs;
pub mod fs;
pub mod image;
pub mod io;
pub mod panic;
pub mod proto;
pub mod rt;

fn wstr(string: &str) -> Box<[u16]> {
    let mut wstring = vec![];
    for c in string.chars() {
        wstring.push(c as u16);
    }
    wstring.push(0);
    wstring.into_boxed_slice()
}

fn load(path: &str) -> Result<Vec<u8>, isize> {
    let wpath = wstr(path);

    for (i, mut fs) in fs::FileSystem::all().iter_mut().enumerate() {
        let mut root = fs.root()?;
        match root.open(&wpath) {
            Ok(mut file) => {
                let mut data = vec![];
                let _count = file.read_to_end(&mut data)?;

                return Ok(data);
            },
            Err(err) => if err != (1 << 63) | 0xE {
                return Err(err);
            }
        }
    }

    Err(0)
}

fn ec() {
    match EcFlash::new(1) {
        Ok(mut ec) => {
            println!("EC FOUND");
            println!("Project: {}", ec.project());
            println!("Version: {}", ec.version());
            println!("Size: {} KB", ec.size()/1024);
        },
        Err(err) => {
            println!("EC ERROR: {}", err);
        }
    }
}

fn exec() {
    let uefi = unsafe { &mut *::UEFI };

    match load("res\\shell.efi") {
        Ok(data) => {
            println!("Start shell");

            let parent_handle = unsafe { ::HANDLE };
            let mut shell_handle = uefi::Handle(0);
            let res = (uefi.BootServices.LoadImage)(false, parent_handle, 0, data.as_ptr(), data.len(), &mut shell_handle);
            println!("Load image: {:X}", res);

            /*
            let arg = [
                b'T' as u16,
                b'E' as u16,
                b'S' as u16,
                b'T' as u16,
                0u16
            ];
            println!("Arg {:X}", arg.as_ptr() as usize);

            let args = [
                arg.as_ptr()
            ];
            println!("Args {:X}", args.as_ptr() as usize);

            let parameters = uefi::shell::ShellParameters {
                Argv: args.as_ptr(),
                Argc: args.len(),
                StdIn: uefi.ConsoleInHandle,
                StdOut: uefi.ConsoleOutHandle,
                StdErr: uefi.ConsoleErrorHandle,
            };
            println!("StdIn: {:X}", parameters.StdIn.0);
            println!("StdOut: {:X}", parameters.StdOut.0);
            println!("StdErr: {:X}", parameters.StdErr.0);
            println!("Parameters: {:X}", &parameters as *const _ as usize);

            // println!("Wait");
            // (uefi.BootServices.Stall)(1000000);

            // let res = (uefi.BootServices.InstallProtocolInterface)(&mut shell_handle, &uefi::guid::EFI_SHELL_PARAMETERS_GUID, uefi::boot::InterfaceType::NativeInterface, &parameters as *const _ as usize);
            // println!("Install parameters: {:X}", res);
            */

            println!("Wait");
            (uefi.BootServices.Stall)(1000000);

            let mut exit_size = 0;
            let mut exit_ptr = ::core::ptr::null_mut();
            let res = (uefi.BootServices.StartImage)(shell_handle, &mut exit_size, &mut exit_ptr);
            println!("Start image: {:X}, {}", res, exit_size);
        },
        Err(err) => {
            println!("Failed to load shell: {:X}", err);
        }
    }
}

fn splash() {
    if let Ok(mut output) = Output::one() {
        let mut max_i = 0;
        let mut max_w = 0;
        let mut max_h = 0;

        for i in 0..output.0.Mode.MaxMode {
            let mut mode_ptr = ::core::ptr::null_mut();
            let mut mode_size = 0;
            (output.0.QueryMode)(output.0, i, &mut mode_size, &mut mode_ptr);

            let mode = unsafe { &mut *mode_ptr };
            let w = mode.HorizontalResolution;
            let h = mode.VerticalResolution;
            if w >= max_w && h >= max_h {
                max_i = i;
                max_w = w;
                max_h = h;
            }
        }

        //(output.0.SetMode)(output.0, max_i);

        let mut display = Display::new(output);

        display.set(Color::rgb(0x41, 0x3e, 0x3c));

        if let Ok(data) = load("res\\splash.bmp") {
            if let Ok(splash) = image::bmp::parse(&data) {
                let x = (display.width() as i32 - splash.width() as i32)/2;
                let y = (display.height() as i32 - splash.height() as i32)/2;
                splash.draw(&mut display, x, y);
            }
        }

        {
            let prompt = "Firmware Updater";
            let mut x = (display.width() as i32 - prompt.len() as i32 * 8)/2;
            let y = display.height() as i32 - 32;
            for c in prompt.chars() {
                display.char(x, y, c, Color::rgb(0xff, 0xff, 0xff));
                x += 8;
            }
        }

        display.sync();
    }
}

fn text() {
    match load("res\\test.txt") {
        Ok(data) => {
            if let Ok(string) = String::from_utf8(data) {
                println!("{}", string);
            } else {
                println!("Failed to parse test file");
            }
        },
        Err(err) => {
            println!("Failed to load test file: {:X}", err);
        }
    }
}

fn main() {
    let uefi = unsafe { &mut *::UEFI };

    loop {
        println!("  1 => ec");
        println!("  2 => exec");
        println!("  3 => splash");
        println!("  4 => text");
        println!("  0 => exit");

        let mut input = uefi::text::TextInputKey {
            ScanCode: 0,
            UnicodeChar: 0
        };

        while input.UnicodeChar == 0 {
            (uefi.ConsoleIn.ReadKeyStroke)(uefi.ConsoleIn, &mut input);
        }

        println!("{}", char::from_u32(input.UnicodeChar as u32).unwrap_or('?'));

        match input.UnicodeChar as u8 {
            b'1' => ec(),
            b'2' => exec(),
            b'3' => splash(),
            b'4' => text(),
            b'0' => return,
            b => println!("Invalid selection '{}'", b as char)
        }
    }
}