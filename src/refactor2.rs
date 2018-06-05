
// // *mut T是一个不变（不变体）类型，所以这里使用*mut T会有问题
// pub struct Vec<T> {
//     ptr: *mut T,  // 分配内存的指针
//     cap: usize,  // 分配内存的大小
//     len: usize,  //  已经被初始化的元素个数
// }
#![feature(allocator_api)]

use std::ptr::{NonNull, self};
use std::mem;
use std::ops::Deref;
use std::ops::DerefMut;
use std::heap::{Alloc, Layout, Global};
use std::alloc::oom;

impl<T> DerefMut for Vec<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe {
            ::std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len)
        }
    }
}

impl<T> Deref for Vec<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        unsafe {
            ::std::slice::from_raw_parts(self.ptr.as_ptr(), self.len)
        }
    }
}

struct IntoIter<T> {
    buf: NonNull<T>,
    cap: usize,
    start: *const T,
    end: *const T,
}

#[derive(Debug)]
pub struct Vec<T> {
    // 使用NonNull<T> 是因为它是T上可变（variant）的
    // 也拥有drop检查
    // 非零指针，可以执行空指针优化
    ptr: NonNull<T>,
    cap: usize,
    len: usize,
}



impl<T> Vec<T> {
    fn new() -> Self {
        assert!(mem::size_of::<T>() != 0, "We're not ready to handle ZSTs");
        Vec { ptr: NonNull::dangling(), len: 0, cap: 0 }
    }


    fn grow(&mut self) {
        // this is all pretty delicate, so let's say it's all unsafe
        unsafe {
            // current API requires us to specify size and alignment manually.

            let (new_cap, ptr) = if self.cap == 0 {
                let ptr = Global.alloc(Layout::array::<T>(1).unwrap());
                (1, ptr)
            } else {
                // as an invariant, we can assume that `self.cap < isize::MAX`,
                // so this doesn't need to be checked.
                let new_cap = self.cap * 2;

                let ptr = Global.realloc(NonNull::from(self.ptr).as_opaque(),
                                       Layout::array::<T>(self.cap).unwrap(),
                                       Layout::array::<T>(new_cap).unwrap().size());
                (new_cap, ptr)
            };

            // If allocate or reallocate fail, we'll get `null` back
            let ptr = match ptr {
               Ok(ptr) => ptr,
               Err(_err) => oom(),
            };

            self.ptr = NonNull::new_unchecked(ptr.as_ptr() as *mut _);
            self.cap = new_cap;
        }
    }

    pub fn push(&mut self, elem: T) {
        if self.len == self.cap { self.grow(); }

        unsafe {
            ptr::write(self.ptr.as_ptr().offset(self.len as isize), elem);
        }

        // Can't fail, we'll OOM first.
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            unsafe {
                Some(ptr::read(self.ptr.as_ptr().offset(self.len as isize)))
            }
        }
    }

    pub fn insert(&mut self, index: usize, elem: T) {
        // Note: `<=` because it's valid to insert after everything
        // which would be equivalent to push.
        assert!(index <= self.len, "index out of bounds");
        if self.cap == self.len { self.grow(); }

        unsafe {
            if index < self.len {
                // ptr::copy(src, dest, len): "copy from source to dest len elems"
                ptr::copy(self.ptr.as_ptr().offset(index as isize),
                          self.ptr.as_ptr().offset(index as isize + 1),
                          self.len - index);
            }
            ptr::write(self.ptr.as_ptr().offset(index as isize), elem);
            self.len += 1;
        }
    }

    pub fn remove(&mut self, index: usize) -> T {
        // Note: `<` because it's *not* valid to remove after everything
        assert!(index < self.len, "index out of bounds");
        unsafe {
            self.len -= 1;
            let result = ptr::read(self.ptr.as_ptr().offset(index as isize));
            ptr::copy(self.ptr.as_ptr().offset(index as isize + 1),
                      self.ptr.as_ptr().offset(index as isize),
                      self.len - index);
            result
        }
    }

    fn into_iter(self) -> IntoIter<T> {
        // Can't destructure Vec since it's Drop
        let ptr = self.ptr;
        let cap = self.cap;
        let len = self.len;

        // Make sure not to drop Vec since that will free the buffer
        mem::forget(self);

        unsafe {
            IntoIter {
                buf: ptr,
                cap: cap,
                start: ptr.as_ptr(),
                end: if cap == 0 {
                    // can't offset off this pointer, it's not allocated!
                    ptr.as_ptr()
                } else {
                    ptr.as_ptr().offset(len as isize)
                }
            }
        }
    }

}

impl<T> Drop for Vec<T> {
    fn drop(&mut self) {
        let elem_size = mem::size_of::<T>();
        if self.cap != 0 && elem_size != 0 {
            unsafe {
                println!("drop!");
                Global.dealloc(NonNull::from(self.ptr).as_opaque(),
                             Layout::array::<T>(self.cap).unwrap());
            }
        }
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.start == self.end {
            None
        } else {
            unsafe {
                let result = ptr::read(self.start);
                self.start = self.start.offset(1);
                Some(result)
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = (self.end as usize - self.start as usize)
                  / mem::size_of::<T>();
        (len, Some(len))
    }

}

impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<T> {
        if self.start == self.end {
            None
        } else {
            unsafe {
                self.end = self.end.offset(-1);
                Some(ptr::read(self.end))
            }
        }
    }
}

fn main(){
    let mut v = Vec::new();
    v.push(1);
    v.push(2);
    println!("{:?}", v);
    for i in v.iter() {
        println!("{:?}", i);
    }
}
