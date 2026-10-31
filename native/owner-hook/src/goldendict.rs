//! Read-only binding profiles for the official GoldenDict 1.5.1 DLLs.
//! Fingerprints cover .text VirtualSize after normalizing known PE relocations.
use crate::trace_protocol::Binding;
use windows_sys::Win32::Foundation::HMODULE;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_NO_MORE_FILES, GetLastError, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::Cryptography::{BCRYPT_SHA256_ALG_HANDLE, BCryptHash};
use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW, TH32CS_SNAPMODULE,
};
use windows_sys::Win32::System::LibraryLoader::GetProcAddress;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
#[path = "goldendict_relocations.rs"]
mod relocations;

#[derive(Debug)]
pub struct Identity {
    pub profile: u32,
    pub digest: [u8; 32],
}
struct Profile {
    id: u32,
    machine: u16,
    timestamp: u32,
    image_size: u32,
    virtual_size: usize,
    raw_size: u32,
    digest: [u8; 32],
    exports: [usize; 4],
    table: usize,
    relocations: &'static [usize],
}
const PROFILES: [Profile; 2] = [
    Profile {
        id: 1,
        machine: 0x8664,
        timestamp: 0x5c53118a,
        image_size: 0xd000,
        virtual_size: 7784,
        raw_size: 8192,
        digest: hex("9e025d94d5d081d22eb91800ebf04534c914fa1ebafe9d8bdbafa05b4ab96a90"),
        exports: [5744, 5696, 5840, 5904],
        table: 0x7020,
        relocations: relocations::RELOC64,
    },
    Profile {
        id: 2,
        machine: 0x14c,
        timestamp: 0xffffffff,
        image_size: 0xc000,
        virtual_size: 6388,
        raw_size: 6656,
        digest: hex("ba1b0d174fb10ad101d232eb567f04940b6bf1e48022a9acd705e54a2debb37b"),
        exports: [6144, 6096, 6256, 6320],
        table: 0x6020,
        relocations: relocations::RELOC32,
    },
];
const fn hex(text: &str) -> [u8; 32] {
    let bytes = text.as_bytes();
    let mut result = [0u8; 32];
    let mut i = 0;
    while i < 64 {
        let value = match bytes[i] {
            b'0'..=b'9' => bytes[i] - b'0',
            b'a'..=b'f' => bytes[i] - b'a' + 10,
            _ => panic!("invalid digest"),
        };
        result[i / 2] = result[i / 2] * 16 + value;
        i += 1;
    }
    result
}
fn read(base: usize, rva: usize, bytes: &mut [u8]) -> Option<()> {
    let address = base.checked_add(rva)?;
    let mut copied = 0;
    let ok = unsafe {
        ReadProcessMemory(
            GetCurrentProcess(),
            address as _,
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            &mut copied,
        )
    };
    (ok != 0 && copied == bytes.len()).then_some(())
}
fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}
fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// The caller retains a balanced loader reference throughout inspection.
pub fn identity(module: HMODULE) -> Option<Identity> {
    let base = module as usize;
    let mut header = [0u8; 4096];
    read(base, 0, &mut header)?;
    if header.get(..2)? != b"MZ" {
        return None;
    }
    let pe = u32_at(&header, 0x3c)? as usize;
    if pe > 0x400 || header.get(pe..pe + 4)? != b"PE\0\0" {
        return None;
    }
    let machine = u16_at(&header, pe + 4)?;
    let opt = pe + 24;
    let profile = PROFILES.iter().find(|p| {
        p.machine == machine
            && u32_at(&header, pe + 8) == Some(p.timestamp)
            && u32_at(&header, opt + 56) == Some(p.image_size)
    })?;
    let preferred = match (machine, u16_at(&header, opt)?) {
        (0x8664, 0x20b) => u64::from_le_bytes(header.get(opt + 24..opt + 32)?.try_into().ok()?),
        (0x14c, 0x10b) => u64::from(u32_at(&header, opt + 28)?),
        _ => return None,
    };
    if preferred != 0x6ad00000 {
        return None;
    }
    let sections = u16_at(&header, pe + 6)? as usize;
    if sections > 16 {
        return None;
    }
    let table = opt + usize::from(u16_at(&header, pe + 20)?);
    let mut found = false;
    for i in 0..sections {
        let entry = table + i * 40;
        if header.get(entry..entry + 8)? == b".text\0\0\0" {
            if found
                || u32_at(&header, entry + 8)? as usize != profile.virtual_size
                || u32_at(&header, entry + 12)? != 0x1000
                || u32_at(&header, entry + 16)? != profile.raw_size
            {
                return None;
            }
            found = true;
        }
    }
    if !found {
        return None;
    }
    let mut text = vec![0u8; profile.virtual_size];
    read(base, 0x1000, &mut text)?;
    let delta = (base as u64).wrapping_sub(preferred);
    // Use the verified original relocation sites, never relocation data supplied
    // by the process. The loader may discard its .reloc pages after loading.
    let mut previous_end = 0x1000;
    for &rva in profile.relocations {
        let width = if machine == 0x8664 { 8 } else { 4 };
        if rva < previous_end {
            return None;
        }
        let offset = rva.checked_sub(0x1000)?;
        let bytes = text.get_mut(offset..offset.checked_add(width)?)?;
        if width == 8 {
            let old = u64::from_le_bytes(bytes.try_into().ok()?);
            bytes.copy_from_slice(&old.wrapping_sub(delta).to_le_bytes());
        } else {
            let old = u32::from_le_bytes(bytes.try_into().ok()?);
            bytes.copy_from_slice(&old.wrapping_sub(delta as u32).to_le_bytes());
        }
        previous_end = rva + width;
    }
    let mut digest = [0u8; 32];
    if unsafe {
        BCryptHash(
            BCRYPT_SHA256_ALG_HANDLE,
            core::ptr::null(),
            0,
            text.as_ptr(),
            text.len() as u32,
            digest.as_mut_ptr(),
            32,
        )
    } < 0
    {
        return None;
    }
    Some(Identity {
        profile: profile.id,
        digest,
    })
}

pub fn snapshot(module: HMODULE) -> Option<(u32, [Binding; 5])> {
    if !single_vendor_module(module) {
        return None;
    }
    let identity = identity(module)?;
    let profile = PROFILES
        .iter()
        .find(|p| p.id == identity.profile && p.digest == identity.digest)?;
    let base = module as usize;
    for (name, rva) in [c"setHook", c"removeHook", c"setHotkeys", c"clearHotkeys"]
        .into_iter()
        .zip(profile.exports)
    {
        let function = unsafe { GetProcAddress(module, name.as_ptr().cast()) }?;
        if function as usize != base.checked_add(rva)? {
            return None;
        }
    }
    if profile.table.checked_add(80)? > profile.image_size as usize {
        return None;
    }
    let read_table = || -> Option<[Binding; 5]> {
        let mut bytes = [0u8; 80];
        read(base, profile.table, &mut bytes)?;
        let mut result = [Binding::default(); 5];
        for (i, binding) in result.iter_mut().enumerate() {
            *binding = Binding {
                key1: u32_at(&bytes, i * 16)?,
                key2: u32_at(&bytes, i * 16 + 4)?,
                modifiers: u32_at(&bytes, i * 16 + 8)?,
            };
            if binding.key1 > 255 || binding.key2 > 255 || binding.modifiers > 15 {
                return None;
            }
        }
        Some(result)
    };
    let first = read_table()?;
    let second = read_table()?;
    (first == second).then_some((profile.id, first))
}

fn single_vendor_module(module: HMODULE) -> bool {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, GetCurrentProcessId()) };
    if snapshot == INVALID_HANDLE_VALUE {
        return false;
    }
    let mut entry = MODULEENTRY32W {
        dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
        ..Default::default()
    };
    let mut count = 0;
    let mut selected = false;
    let mut present = unsafe { Module32FirstW(snapshot, &mut entry) } != 0;
    while present {
        let length = entry
            .szModule
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(entry.szModule.len());
        let name = &entry.szModule[..length];
        let expected = b"gdhotkeys.dll";
        if name.len() == expected.len()
            && name
                .iter()
                .zip(expected)
                .all(|(&a, &b)| a <= 127 && (a as u8).to_ascii_lowercase() == b)
        {
            count += 1;
            selected |= entry.hModule == module;
        }
        present = unsafe { Module32NextW(snapshot, &mut entry) } != 0;
    }
    let complete = unsafe { GetLastError() } == ERROR_NO_MORE_FILES;
    unsafe { CloseHandle(snapshot) };
    complete && count == 1 && selected
}
