use std::{
    any,
    cell::UnsafeCell,
    mem::{self, ManuallyDrop},
    ops::{Deref, DerefMut},
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Debug)]
pub struct AtomicRefCell<T: ?Sized> {
    borrow: AtomicUsize,
    data: UnsafeCell<T>,
}

impl<T> AtomicRefCell<T> {
    pub fn new(data: T) -> Self {
        Self {
            borrow: AtomicUsize::new(0),
            data: UnsafeCell::new(data),
        }
    }

    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }

    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }

    pub fn replace(&self, t: T) -> T {
        mem::replace(&mut *self.borrow_mut(), t)
    }

    pub fn replace_with<F: FnOnce(&mut T) -> T>(&self, f: F) -> T {
        let borrow = &mut *self.borrow_mut();
        let result = f(borrow);
        mem::replace(borrow, result)
    }

    pub fn swap(&self, other: &AtomicRefCell<T>) {
        mem::swap(&mut *self.borrow_mut(), &mut *other.borrow_mut())
    }
}

impl<T: ?Sized> AtomicRefCell<T> {
    const MUTABLY_BORROWED: usize = usize::MAX;

    pub fn borrow(&self) -> AtomicRef<'_, T> {
        self.try_borrow()
            .unwrap_or_else(|| panic!("{} was already borrowed mutably!", any::type_name::<T>()))
    }

    pub fn try_borrow(&self) -> Option<AtomicRef<'_, T>> {
        loop {
            let borrow_state = self.borrow.load(Ordering::Acquire);
            let old =
                self.borrow
                    .compare_and_swap(borrow_state, borrow_state + 1, Ordering::AcqRel);

            if old == Self::MUTABLY_BORROWED {
                return None;
            } else if old == borrow_state {
                break;
            }
        }

        let data = unsafe { &*self.data.get() };

        Some(AtomicRef {
            flag: &self.borrow,
            data,
        })
    }

    pub unsafe fn borrow_unchecked(&self) -> AtomicRef<'_, T> {
        let old = self.borrow.fetch_add(1, Ordering::AcqRel);
        debug_assert_ne!(old, Self::MUTABLY_BORROWED);

        let data = &*self.data.get();

        AtomicRef {
            flag: &self.borrow,
            data,
        }
    }

    pub fn borrow_mut(&self) -> AtomicRefMut<'_, T> {
        self.try_borrow_mut()
            .unwrap_or_else(|| panic!("{} was already borrowed!", any::type_name::<T>()))
    }

    pub fn try_borrow_mut(&self) -> Option<AtomicRefMut<'_, T>> {
        if self
            .borrow
            .compare_and_swap(0, Self::MUTABLY_BORROWED, Ordering::AcqRel)
            != 0
        {
            return None;
        }

        let data = unsafe { &mut *self.data.get() };

        Some(AtomicRefMut {
            flag: &self.borrow,
            data,
        })
    }

    pub unsafe fn borrow_mut_unchecked(&self) -> AtomicRefMut<'_, T> {
        let old = self.borrow.swap(Self::MUTABLY_BORROWED, Ordering::AcqRel);
        debug_assert_eq!(old, 0);

        let data = &mut *self.data.get();

        AtomicRefMut {
            flag: &self.borrow,
            data,
        }
    }

    pub fn as_ptr(&self) -> *mut T {
        self.data.get()
    }
}

unsafe impl<T: ?Sized + Send> Send for AtomicRefCell<T> {}

unsafe impl<T: ?Sized + Send> Sync for AtomicRefCell<T> {}

#[derive(Debug)]
pub struct AtomicRef<'a, T: ?Sized> {
    flag: &'a AtomicUsize,
    pub(crate) data: &'a T,
}

impl<'a, T: ?Sized> AtomicRef<'a, T> {
    pub fn map<U, F>(this: Self, f: F) -> AtomicRef<'a, U>
    where
        F: FnOnce(&T) -> &U,
        U: ?Sized,
    {
        let flag = this.flag;
        let data = f(this.data);

        mem::forget(this);

        AtomicRef { flag, data }
    }
}

impl<T: ?Sized> Deref for AtomicRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.data
    }
}

impl<T: ?Sized> Drop for AtomicRef<'_, T> {
    fn drop(&mut self) {
        let old_val = self.flag.fetch_sub(1, Ordering::AcqRel);
        debug_assert_ne!(old_val, AtomicRefCell::<T>::MUTABLY_BORROWED);
    }
}

#[derive(Debug)]
pub struct AtomicRefMut<'a, T: ?Sized> {
    flag: &'a AtomicUsize,
    pub(crate) data: &'a mut T,
}

impl<'a, T: ?Sized> AtomicRefMut<'a, T> {
    pub fn map<U, F>(this: Self, f: F) -> AtomicRefMut<'a, U>
    where
        F: FnOnce(&mut T) -> &mut U,
        U: ?Sized,
    {
        let this = ManuallyDrop::new(this);

        let flag = this.flag;
        // SAFETY: This is just used as a way to move out
        // of the AtomicRefMut without dropping its contents,
        // which would cause it to modify the flag erroneously.
        // If done like RefMut in the normal RefCell, rustc gives
        // an error about moving out of `this`.
        let data = unsafe { f(ptr::read(&this.data)) };

        AtomicRefMut { flag, data }
    }
}

impl<T: ?Sized> Deref for AtomicRefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.data
    }
}

impl<T: ?Sized> DerefMut for AtomicRefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.data
    }
}

impl<T: ?Sized> Drop for AtomicRefMut<'_, T> {
    fn drop(&mut self) {
        let old_val = self.flag.swap(0, Ordering::Release);
        debug_assert_eq!(old_val, AtomicRefCell::<T>::MUTABLY_BORROWED);
    }
}
