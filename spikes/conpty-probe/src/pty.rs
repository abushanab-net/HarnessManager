use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr::{null, null_mut};

use anyhow::{Context, Result, ensure};
use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Console::{
    COORD, ClosePseudoConsole, CreatePseudoConsole, HPCON, ResizePseudoConsole,
};
use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::*;

use crate::win_ok;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

impl Size {
    fn coord(self) -> COORD {
        COORD {
            X: self.cols as i16,
            Y: self.rows as i16,
        }
    }
}

pub struct Pty(HPCON);

impl Pty {
    pub fn open(size: Size) -> Result<(Self, File, File)> {
        let (input, writer) = pipe()?;
        let (reader, output) = pipe()?;
        let mut pty = 0;
        let result = unsafe {
            CreatePseudoConsole(
                size.coord(),
                input.as_raw_handle(),
                output.as_raw_handle(),
                0,
                &mut pty,
            )
        };
        ensure!(result >= 0, "CreatePseudoConsole failed ({result:#x})");
        Ok((Self(pty), reader, writer))
    }

    pub fn resize(&self, size: Size) -> Result<()> {
        let result = unsafe { ResizePseudoConsole(self.0, size.coord()) };
        ensure!(result >= 0, "ResizePseudoConsole failed ({result:#x})");
        Ok(())
    }

    pub fn spawn(&self, args: &[OsString], job: HANDLE) -> Result<OwnedHandle> {
        let attributes = Attributes::new(self.0)?;
        let mut start = STARTUPINFOEXW::default();
        start.StartupInfo.cb = std::mem::size_of_val(&start) as u32;
        // Let ConPTY supply all three handles even when the parent has redirected stdio.
        start.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        start.StartupInfo.hStdInput = INVALID_HANDLE_VALUE;
        start.StartupInfo.hStdOutput = INVALID_HANDLE_VALUE;
        start.StartupInfo.hStdError = INVALID_HANDLE_VALUE;
        start.lpAttributeList = attributes.ptr();
        let mut process = PROCESS_INFORMATION::default();
        let mut cmdline = command_line(args);
        let mut exe: Vec<u16> = args[0].encode_wide().collect();
        exe.push(0);
        unsafe {
            win_ok(CreateProcessW(
                exe.as_ptr(),
                cmdline.as_mut_ptr(),
                null(),
                null(),
                0,
                CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT,
                null(),
                null(),
                &start.StartupInfo,
                &mut process,
            ))
            .context("start ConPTY child")?;
            let child = OwnedHandle::from_raw_handle(process.hProcess);
            let thread = OwnedHandle::from_raw_handle(process.hThread);
            if let Err(err) = win_ok(AssignProcessToJobObject(job, child.as_raw_handle())) {
                TerminateProcess(child.as_raw_handle(), 1);
                return Err(err).context("assign ConPTY child to cleanup job");
            }
            win_ok((ResumeThread(thread.as_raw_handle()) != u32::MAX) as i32)
                .context("resume ConPTY child")?;
            Ok(child)
        }
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        unsafe { ClosePseudoConsole(self.0) };
    }
}

struct Attributes(Vec<usize>);

impl Attributes {
    fn new(pty: HPCON) -> io::Result<Self> {
        let mut size = 0;
        unsafe { InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut size) };
        let mut storage = vec![0; size.div_ceil(std::mem::size_of::<usize>())];
        unsafe {
            win_ok(InitializeProcThreadAttributeList(
                storage.as_mut_ptr().cast(),
                1,
                0,
                &mut size,
            ))?
        };
        let attributes = Self(storage);
        unsafe {
            win_ok(UpdateProcThreadAttribute(
                attributes.ptr(),
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                pty as _,
                std::mem::size_of::<HPCON>(),
                null_mut(),
                null(),
            ))?;
        }
        Ok(attributes)
    }

    fn ptr(&self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.0.as_ptr().cast_mut().cast()
    }
}

impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.ptr()) };
    }
}

fn pipe() -> io::Result<(File, File)> {
    let mut reader = null_mut();
    let mut writer = null_mut();
    unsafe {
        win_ok(CreatePipe(&mut reader, &mut writer, null(), 0))?;
        Ok((File::from_raw_handle(reader), File::from_raw_handle(writer)))
    }
}

pub fn exit_code(child: &OwnedHandle) -> io::Result<Option<u32>> {
    match unsafe { WaitForSingleObject(child.as_raw_handle(), 0) } {
        WAIT_TIMEOUT => Ok(None),
        WAIT_OBJECT_0 => {
            let mut code = 0;
            unsafe { win_ok(GetExitCodeProcess(child.as_raw_handle(), &mut code))? };
            Ok(Some(code))
        }
        _ => Err(io::Error::last_os_error()),
    }
}

fn command_line(args: &[OsString]) -> Vec<u16> {
    const QUOTE: u16 = b'"' as u16;
    const SLASH: u16 = b'\\' as u16;
    const SPACE: u16 = b' ' as u16;
    const TAB: u16 = b'\t' as u16;
    let mut command = Vec::new();
    for arg in args {
        if !command.is_empty() {
            command.push(SPACE);
        }
        if !arg.is_empty()
            && !arg
                .encode_wide()
                .any(|unit| matches!(unit, SPACE | TAB | QUOTE))
        {
            command.extend(arg.encode_wide());
            continue;
        }
        command.push(QUOTE);
        let mut slashes = 0;
        for unit in arg.encode_wide() {
            if unit == SLASH {
                slashes += 1;
                continue;
            }
            let count = if unit == QUOTE {
                2 * slashes + 1
            } else {
                slashes
            };
            command.extend(std::iter::repeat_n(SLASH, count));
            slashes = 0;
            command.push(unit);
        }
        command.extend(std::iter::repeat_n(SLASH, 2 * slashes));
        command.push(QUOTE);
    }
    command.push(0);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_spaces_empty_arguments_quotes_and_trailing_slashes() {
        let args = [
            "program.exe",
            "",
            r"C:\path with space\",
            r#"contains"quote"#,
        ]
        .map(OsString::from);
        let actual = String::from_utf16(&command_line(&args)).unwrap();
        assert_eq!(
            actual.trim_end_matches('\0'),
            r#"program.exe "" "C:\path with space\\" "contains\"quote""#
        );
    }
}
