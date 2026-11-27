//! Les bibliothèques du téléviseur (décodeur NDL, chaîne Dolby, Luna) : ouvertes par `dlopen`, jamais liées, pour
//! que l'app démarre sur un téléviseur qui n'en a pas une (WEBOS-27 : une seule copie de ces deux appels).

use std::ffi::{c_void, CString};

/// Ouvre une bibliothèque du téléviseur (`RTLD_NOW | RTLD_GLOBAL`) ; `None` si elle manque.
pub fn open(name: &str) -> Option<*mut c_void> {
    let name = CString::new(name).ok()?;
    let lib = unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL) };
    (!lib.is_null()).then_some(lib)
}

/// Un symbole de `lib`, vu comme `T` ; `None` s'il manque.
///
/// # Safety
///
/// `T` est un pointeur de fonction (`unsafe extern "C" fn …`) à la signature exacte du symbole.
pub unsafe fn sym<T>(lib: *mut c_void, name: &str) -> Option<T> {
    let n = CString::new(name).ok()?;
    let p = unsafe { libc::dlsym(lib, n.as_ptr()) };
    if p.is_null() { None } else { Some(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&p) }) }
}
