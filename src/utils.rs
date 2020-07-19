#[cfg(not(debug_assertions))]
use std::hint::unreachable_unchecked;

// While technically this doesn't have to be marked unsafe, it prevents rustc from
// complaining that there is an unneeded unsafe block around it, which is required
// in release mode.
/// A function which panics with an unreachable error in non-release modes, but which
/// simply calls [`unreachable_unchecked`] in release mode to enable further
/// optimizations. Only call this when you are able to ensure that the branch containing
/// a call to this function code can never as calling it in release mode is undefined
/// behavior.
#[cfg(debug_assertions)]
pub unsafe fn debug_unreachable(msg: &'static str) -> ! {
    if msg.trim() == "" {
        unreachable!();
    } else {
        unreachable!(msg)
    }
}

/// A function which panics with an unreachable error in non-release modes, but which
/// simply calls [`unreachable_unchecked`] in release mode to enable further
/// optimizations. Only call this when you are able to ensure that the branch containing
/// a call to this function code can never as calling it in release mode is undefined
/// behavior.
#[cfg(not(debug_assertions))]
pub unsafe fn debug_unreachable(_: &'static str) -> ! {
    unreachable_unchecked();
}

/// A function which calls a closure that never returns in non-release modes, but
/// which simply calls [`unreachable_unchecked`] in release mode to enable further
/// optimizations. Only call this when you are able to ensure that the branch containing
/// a call to this function code can never as calling it in release mode is undefined
/// behavior.
#[cfg(debug_assertions)]
pub unsafe fn debug_closure<F: FnOnce()>(f: F) -> ! {
    f();
    unreachable!();
}

/// A function which calls a closure that never returns in non-release modes, but
/// which simply calls [`unreachable_unchecked`] in release mode to enable further
/// optimizations. Only call this when you are able to ensure that the branch containing
/// a call to this function code can never as calling it in release mode is undefined
/// behavior.
#[cfg(not(debug_assertions))]
pub unsafe fn debug_closure<F: FnOnce()>(_: F) -> ! {
    unreachable_unchecked();
}
