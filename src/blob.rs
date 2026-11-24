use windows::{
    Win32::{
        Foundation::E_POINTER,
        System::LibraryLoader::{FindResourceW, LoadResource, LockResource, SizeofResource},
    },
    core::{Error, PCWSTR, Result},
};

use crate::pass_text;

pub const fn int_resource(id: u16) -> PCWSTR {
    PCWSTR::from_raw(id as usize as *const u16)
}

struct ResourceId {
    lpname: PCWSTR,
}

impl<'a> Into<ResourceId> for &'a str {
    fn into(self) -> ResourceId {
        ResourceId {
            lpname: pass_text(self),
        }
    }
}

impl Into<ResourceId> for u16 {
    fn into(self) -> ResourceId {
        ResourceId {
            lpname: int_resource(self),
        }
    }
}

// ref: https://learn.microsoft.com/en-us/windows/win32/menurc/resource-types
const RT_RCDATA: PCWSTR = int_resource(10);

#[allow(private_bounds)]
pub fn load_resouse_blob(id: impl Into<ResourceId>) -> Result<&'static [u8]> {
    let resource_id = id.into();
    let hres = unsafe { FindResourceW(None, resource_id.lpname, RT_RCDATA) };

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
