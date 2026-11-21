use std::{ptr, slice};
use windows_sys::Win32::{
    Foundation::{ERROR_CANCELLED, ERROR_NOT_FOUND, GetLastError, HWND},
    Security::Credentials::*,
};
use zeroize::{Zeroize, Zeroizing};

use super::{ProviderError, Result};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

pub fn target(namespace: &str, id: &str) -> String {
    format!("{namespace}/provider/{id}")
}

pub fn prompt(owner: usize, endpoint: &str) -> Result<Option<Zeroizing<Vec<u8>>>> {
    let caption = wide("ngodik — API key");
    let message = wide(&format!(
        "Tempel API key untuk {endpoint} pada kolom Password."
    ));
    let target = wide("ngodik/APIKeyEntry");
    let mut username = [0u16; 64];
    username[..6].copy_from_slice(&wide("ngodik")[..6]);
    let mut password = Zeroizing::new([0u16; 513]);
    let info = CREDUI_INFOW {
        cbSize: size_of::<CREDUI_INFOW>() as u32,
        hwndParent: owner as HWND,
        pszMessageText: message.as_ptr(),
        pszCaptionText: caption.as_ptr(),
        hbmBanner: ptr::null_mut(),
    };
    let mut save = 0;
    // Buffers remain valid for the synchronous native modal dialog.
    let result = unsafe {
        CredUIPromptForCredentialsW(
            &info,
            target.as_ptr(),
            ptr::null(),
            0,
            username.as_mut_ptr(),
            username.len() as u32,
            password.as_mut_ptr(),
            password.len() as u32,
            &mut save,
            CREDUI_FLAGS_GENERIC_CREDENTIALS
                | CREDUI_FLAGS_ALWAYS_SHOW_UI
                | CREDUI_FLAGS_DO_NOT_PERSIST
                | CREDUI_FLAGS_KEEP_USERNAME,
        )
    };
    if result == ERROR_CANCELLED {
        return Ok(None);
    }
    if result != 0 {
        return Err(ProviderError::CredentialUnavailable);
    }

    let mut key = Zeroizing::new(Vec::new());
    for &unit in password.iter().take_while(|&&unit| unit != 0) {
        // Restrict the key to the printable ASCII allowed in a Bearer token.
        if !(0x21..=0x7e).contains(&unit) {
            return Err(ProviderError::InvalidKey);
        }
        key.push(unit as u8);
    }
    if key.is_empty() {
        return Err(ProviderError::InvalidKey);
    }
    Ok(Some(key))
}

pub fn write(target: &str, key: &mut [u8]) -> Result<()> {
    let mut name = wide(target);
    let mut username = wide("ngodik");
    let record = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: name.as_mut_ptr(),
        CredentialBlobSize: key.len() as u32,
        CredentialBlob: key.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: username.as_mut_ptr(),
        ..Default::default()
    };
    // Windows copies the credential during this call.
    if unsafe { CredWriteW(&record, 0) } == 0 {
        return Err(ProviderError::CredentialUnavailable);
    }
    Ok(())
}

pub fn read(target: &str) -> Result<Option<Zeroizing<Vec<u8>>>> {
    let target = wide(target);
    let mut record = ptr::null_mut();
    // CredRead owns this allocation until the matching CredFree.
    unsafe {
        if CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut record) == 0 {
            return if GetLastError() == ERROR_NOT_FOUND {
                Ok(None)
            } else {
                Err(ProviderError::CredentialUnavailable)
            };
        }
        let key = if (*record).CredentialBlobSize == 0 {
            None
        } else {
            let bytes = slice::from_raw_parts_mut(
                (*record).CredentialBlob,
                (*record).CredentialBlobSize as usize,
            );
            let key = Zeroizing::new(bytes.to_vec());
            bytes.zeroize();
            Some(key)
        };
        CredFree(record.cast());
        Ok(key)
    }
}

pub fn delete(target: &str) -> Result<()> {
    let target = wide(target);
    // Treat an already removed credential as a successful deletion.
    if unsafe {
        CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) == 0 && GetLastError() != ERROR_NOT_FOUND
    } {
        return Err(ProviderError::CredentialUnavailable);
    }
    Ok(())
}
