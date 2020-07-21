use std::os::raw::c_void;
use std::ptr::NonNull;

use crate::{ffi, ot, Script};

pub struct ShapePlan {
    #[allow(dead_code)]
    plan: NonNull<ffi::hb_ot_shape_plan_t>,
    pub ot_map: ot::Map,
}

impl ShapePlan {
    pub fn from_ptr(ptr: *const ffi::hb_ot_shape_plan_t) -> Self {
        assert!(!ptr.is_null());
        unsafe {
            ShapePlan {
                plan: NonNull::new(ptr as _).unwrap(),
                ot_map: ot::Map::from_ptr(ffi::hb_ot_shape_plan_get_ot_map(ptr)),
            }
        }
    }

    pub fn data(&self) -> *const c_void {
        unsafe {
            ffi::hb_ot_shape_plan_get_data(self.plan.as_ptr())
        }
    }

    pub fn script(&self) -> Script {
        unsafe {
            Script::from_raw(ffi::hb_ot_shape_plan_get_script(self.plan.as_ptr()))
        }
    }

    pub fn has_gpos_mark(&self) -> bool {
        unsafe {
            ffi::hb_ot_shape_plan_has_gpos_mark(self.plan.as_ptr())
        }
    }
}
