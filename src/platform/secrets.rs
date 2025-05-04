use anyhow::{Result, ensure};
use std::ptr::{null, null_mut};
use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::*};

pub fn protect(bytes: &[u8]) -> Result<Vec<u8>> {
    crypt(bytes, true)
}
pub fn unprotect(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.is_empty() {
        Ok(vec![])
    } else {
        crypt(bytes, false)
    }
}
fn crypt(bytes: &[u8], protect: bool) -> Result<Vec<u8>> {
    ensure!(bytes.len() < 64 * 1024, "API token is too large");
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as _,
    };
    let mut output: CRYPT_INTEGER_BLOB = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        if protect {
            CryptProtectData(
                &input,
                null(),
                null(),
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                null_mut(),
                null(),
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    ensure!(
        ok != 0,
        "Windows could not protect/unprotect the API token for this user"
    );
    let result =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        std::ptr::write_bytes(output.pbData, 0, output.cbData as usize);
        LocalFree(output.pbData.cast());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dpapi_roundtrips_only_synthetic_token_and_rejects_invalid_blob() {
        let plaintext = b"public-fixture-token-not-a-real-secret";
        let protected = protect(plaintext).unwrap();
        assert_ne!(protected, plaintext);
        assert_eq!(unprotect(&protected).unwrap(), plaintext);
        assert!(unprotect(&[1, 2, 3, 4]).is_err());
    }
}
