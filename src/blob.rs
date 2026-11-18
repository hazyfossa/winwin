use windows::{
    Win32::{
        Foundation::E_POINTER,
        System::LibraryLoader::{FindResourceW, LoadResource, LockResource, SizeofResource},
    },
    core::{Error, PCWSTR, Result},
};

use crate::pass_text;

pub enum ResourceId<'a> {
    Text(&'a str),
    Int(u16),
}

pub const fn int_resource(id: u16) -> PCWSTR {
    PCWSTR::from_raw(id as usize as *const u16)
}

impl<'a> ResourceId<'a> {
    fn into_lpname(self) -> PCWSTR {
        match self {
            Self::Int(x) => int_resource(x),
            Self::Text(x) => pass_text(x),
        }
    }
}

// ref: https://learn.microsoft.com/en-us/windows/win32/menurc/resource-types
const RT_RCDATA: PCWSTR = int_resource(10);

pub fn load_resouse_blob(id: ResourceId) -> Result<&'static [u8]> {
    let lpname = id.into_lpname();
    let hres = unsafe { FindResourceW(None, lpname, RT_RCDATA) };

    if hres.is_invalid() {
        return Err(Error::from_thread());
    }

    let len = unsafe { SizeofResource(None, hres) } as usize;
    let hmem = unsafe { LoadResource(None, hres)? };

    let ptr = unsafe { LockResource(hmem) };

    if ptr.is_null() {
        return Err(Error::from_hresult(E_POINTER));
    }

    if len == 0 {
        return Ok(&[]);
    }

    // SAFETY: `data` is non-null, `u8`-aligned, and points to `len`
    // initialized, immutable bytes in the mapped image.
    //
    // We always load from the current module, so the 'static lifetime is appropriate:
    // windows will not clean up current resources while the program is running
    let bytes = unsafe { std::slice::from_raw_parts(ptr.cast(), len) };
    Ok(bytes)
}
