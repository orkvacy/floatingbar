//! Per-user Windows DPAPI storage. Only encrypted bytes are written to disk.
use serde::{de::DeserializeOwned, Serialize};
use std::{fs, path::Path};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

pub fn protect(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len().try_into().map_err(|_| "Data terlalu besar")?,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        let result = if encrypt {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        result.map_err(|_| "Data privat tidak bisa dibuka oleh akun Windows ini".to_string())?;
        let data = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(output.pbData as *mut _));
        Ok(data)
    }
}

pub fn read<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|_| "Data privat belum tersedia atau tidak bisa dibaca")?;
    serde_json::from_slice(&protect(&bytes, false)?)
        .map_err(|_| "Format data privat tidak valid".into())
}

pub fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    fs::create_dir_all(path.parent().ok_or("Lokasi penyimpanan tidak tersedia")?)
        .map_err(|_| "Folder data privat tidak bisa dibuat")?;
    let bytes = protect(
        &serde_json::to_vec(value).map_err(|_| "Data privat tidak valid")?,
        true,
    )?;
    let temp = path.with_extension("tmp");
    fs::write(&temp, bytes).map_err(|_| "Data privat tidak bisa disimpan")?;
    let result = fs::rename(&temp, path);
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.map_err(|_| "Data privat tidak bisa disimpan".into())
}
