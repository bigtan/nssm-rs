use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR};
use windows::Win32::System::Registry::*;
use windows::core::{PCWSTR, PWSTR};

use crate::error::{AppError, AppResult};

pub struct RegistryKey {
    handle: HKEY,
    path: String,
}

impl RegistryKey {
    pub fn create_local_machine(path: &str, access: REG_SAM_FLAGS) -> AppResult<Self> {
        let mut handle = HKEY::default();
        let path_wide = to_wide(path);
        let result = unsafe {
            RegCreateKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR::from_raw(path_wide.as_ptr()),
                Some(0),
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                access,
                None,
                &mut handle,
                None,
            )
        };

        if result != WIN32_ERROR(0) {
            return Err(AppError::Registry {
                operation: "create",
                path: path.to_string(),
                code: result.0,
            });
        }

        Ok(Self {
            handle,
            path: path.to_string(),
        })
    }

    pub fn open_local_machine(path: &str, access: REG_SAM_FLAGS) -> AppResult<Self> {
        let mut handle = HKEY::default();
        let path_wide = to_wide(path);
        let result = unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR::from_raw(path_wide.as_ptr()),
                Some(0),
                access,
                &mut handle,
            )
        };

        if result != WIN32_ERROR(0) {
            return Err(AppError::Registry {
                operation: "open",
                path: path.to_string(),
                code: result.0,
            });
        }

        Ok(Self {
            handle,
            path: path.to_string(),
        })
    }

    pub fn delete_tree_local_machine(path: &str) -> AppResult<()> {
        let path_wide = to_wide(path);
        let result =
            unsafe { RegDeleteTreeW(HKEY_LOCAL_MACHINE, PCWSTR::from_raw(path_wide.as_ptr())) };

        if result != WIN32_ERROR(0) && result != ERROR_FILE_NOT_FOUND {
            return Err(AppError::Registry {
                operation: "delete",
                path: path.to_string(),
                code: result.0,
            });
        }

        Ok(())
    }

    pub fn set_string(&self, name: &str, value: &str) -> AppResult<()> {
        let name_wide = to_wide(name);
        let value_wide = to_wide(value);
        let bytes = unsafe {
            std::slice::from_raw_parts(value_wide.as_ptr() as *const u8, value_wide.len() * 2)
        };
        let result = unsafe {
            RegSetValueExW(
                self.handle,
                PCWSTR::from_raw(name_wide.as_ptr()),
                Some(0),
                REG_SZ,
                Some(bytes),
            )
        };

        if result != WIN32_ERROR(0) {
            return Err(AppError::Registry {
                operation: "set string",
                path: format!("{}\\{}", self.path, name),
                code: result.0,
            });
        }

        Ok(())
    }

    pub fn set_dword(&self, name: &str, value: u32) -> AppResult<()> {
        let name_wide = to_wide(name);
        let bytes =
            unsafe { std::slice::from_raw_parts((&value as *const u32) as *const u8, 4usize) };
        let result = unsafe {
            RegSetValueExW(
                self.handle,
                PCWSTR::from_raw(name_wide.as_ptr()),
                Some(0),
                REG_DWORD,
                Some(bytes),
            )
        };

        if result != WIN32_ERROR(0) {
            return Err(AppError::Registry {
                operation: "set dword",
                path: format!("{}\\{}", self.path, name),
                code: result.0,
            });
        }

        Ok(())
    }

    pub fn get_string(&self, name: &str) -> AppResult<Option<String>> {
        let name_wide = to_wide(name);
        let mut value_type = REG_VALUE_TYPE(0);
        let mut size = 0u32;
        let query_result = unsafe {
            RegQueryValueExW(
                self.handle,
                PCWSTR::from_raw(name_wide.as_ptr()),
                None,
                Some(&mut value_type),
                None,
                Some(&mut size),
            )
        };

        if query_result == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if query_result != WIN32_ERROR(0) {
            return Err(AppError::Registry {
                operation: "query string size",
                path: format!("{}\\{}", self.path, name),
                code: query_result.0,
            });
        }

        let mut buffer = vec![0u8; size as usize];
        let result = unsafe {
            RegQueryValueExW(
                self.handle,
                PCWSTR::from_raw(name_wide.as_ptr()),
                None,
                Some(&mut value_type),
                Some(buffer.as_mut_ptr()),
                Some(&mut size),
            )
        };

        if result != WIN32_ERROR(0) {
            return Err(AppError::Registry {
                operation: "get string",
                path: format!("{}\\{}", self.path, name),
                code: result.0,
            });
        }

        let wide_len = size as usize / 2;
        let wide = unsafe { std::slice::from_raw_parts(buffer.as_ptr() as *const u16, wide_len) };
        let mut text = String::from_utf16_lossy(wide);
        while text.ends_with('\0') {
            text.pop();
        }
        Ok(Some(text))
    }

    pub fn get_dword(&self, name: &str) -> AppResult<Option<u32>> {
        let name_wide = to_wide(name);
        let mut value = 0u32;
        let mut size = 4u32;
        let result = unsafe {
            RegQueryValueExW(
                self.handle,
                PCWSTR::from_raw(name_wide.as_ptr()),
                None,
                None,
                Some((&mut value as *mut u32) as *mut u8),
                Some(&mut size),
            )
        };

        if result == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if result != WIN32_ERROR(0) {
            return Err(AppError::Registry {
                operation: "get dword",
                path: format!("{}\\{}", self.path, name),
                code: result.0,
            });
        }

        Ok(Some(value))
    }

    pub fn enum_subkeys(&self) -> AppResult<Vec<String>> {
        let mut names = Vec::new();
        let mut index = 0u32;

        loop {
            let mut name_len = 256u32;
            let mut name = vec![0u16; name_len as usize];
            let result = unsafe {
                RegEnumKeyExW(
                    self.handle,
                    index,
                    Some(PWSTR::from_raw(name.as_mut_ptr())),
                    &mut name_len,
                    None,
                    Some(PWSTR::null()),
                    None,
                    None,
                )
            };

            if result == ERROR_FILE_NOT_FOUND {
                break;
            }
            if result.0 == 259 {
                break;
            }
            if result != WIN32_ERROR(0) {
                return Err(AppError::Registry {
                    operation: "enum subkeys",
                    path: self.path.clone(),
                    code: result.0,
                });
            }

            names.push(String::from_utf16_lossy(&name[..name_len as usize]));
            index += 1;
        }

        Ok(names)
    }
}

impl Drop for RegistryKey {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.handle);
        }
    }
}

pub fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
