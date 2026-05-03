use windows::Win32::Foundation::{CloseHandle, HANDLE, RPC_E_CHANGED_MODE};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::Registry::{HKEY, RegCloseKey};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, DestroyMenu, HICON, HMENU};
use windows::core::Error as WinError;

use crate::error::Result;

pub struct CoApartment {
    initialized: bool,
}

impl CoApartment {
    pub fn init() -> Result<Self> {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if hr.is_err() {
            if hr != RPC_E_CHANGED_MODE {
                return Err(WinError::from_hresult(hr).into());
            }
            return Ok(Self { initialized: false });
        }
        Ok(Self { initialized: true })
    }
}

impl Drop for CoApartment {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { CoUninitialize() };
        }
    }
}

pub struct Handle(HANDLE);

impl Handle {
    pub fn new(handle: HANDLE) -> Option<Self> {
        if handle.is_invalid() || handle.0.is_null() {
            None
        } else {
            Some(Self(handle))
        }
    }

    pub fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub struct RegKey(HKEY);

impl RegKey {
    pub fn new(key: HKEY) -> Self {
        Self(key)
    }

    pub fn raw(&self) -> HKEY {
        self.0
    }
}

impl Drop for RegKey {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

pub struct OwnedMenu(pub HMENU);

impl Drop for OwnedMenu {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyMenu(self.0);
        }
    }
}

pub struct OwnedIcon(pub HICON);

impl Drop for OwnedIcon {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyIcon(self.0);
        }
    }
}
