use core::fmt;
use core::nonzero::NonZero;
use core::mem::{size_of, transmute};
use libc;
use ffi;
use python::{Python, PythonObject, PythonObjectWithCheckedDowncast, PythonObjectWithTypeObject, PythonObjectDowncastError, ToPythonPointer};
use objects::PyType;
use err::{PyErr, PyResult};

#[unsafe_no_drop_flag]
pub struct PyObject<'p> {
    // PyObject<'p> owns one reference to the *PyObject
    // ptr is not null (except possibly due to #[unsafe_no_drop_flag])
    ptr: NonZero<*mut ffi::PyObject>,
    py : Python<'p>
}

#[unsafe_destructor]
impl <'p> Drop for PyObject<'p> {
    fn drop(&mut self) {
        // TODO: change from Py_XDECREF to Py_DECREF when #[unsafe_no_drop_flag] disappears
        unsafe { ffi::Py_XDECREF(*self.ptr); }
    }
}

impl <'p> Clone for PyObject<'p> {
    #[inline]
    fn clone(&self) -> PyObject<'p> {
        unsafe { ffi::Py_INCREF(*self.ptr) };
        PyObject { ptr: self.ptr, py: self.py }
    }
}

impl <'p> PythonObject<'p> for PyObject<'p> {
    #[inline]
    fn as_object<'a>(&'a self) -> &'a PyObject<'p> {
        self
    }
    
    #[inline]
    fn into_object(self) -> PyObject<'p> {
        self
    }
    
    #[inline]
    fn unchecked_downcast_from(o: PyObject<'p>) -> PyObject<'p> {
        o
    }
    
    #[inline]
    fn unchecked_downcast_borrow_from<'a>(o: &'a PyObject<'p>) -> &'a PyObject<'p> {
        o
    }
    
    #[inline]
    fn python(&self) -> Python<'p> {
        self.py
    }
}

impl <'p> PythonObjectWithCheckedDowncast<'p> for PyObject<'p> {
    #[inline]
    fn downcast_from(obj: PyObject<'p>) -> Result<PyObject<'p>, PythonObjectDowncastError<'p>> {
        Ok(obj)
    }
    
    #[inline]
    fn downcast_borrow_from<'a>(obj: &'a PyObject<'p>) -> Result<&'a PyObject<'p>, PythonObjectDowncastError<'p>> {
        Ok(obj)
    }
}

impl <'p> PythonObjectWithTypeObject<'p> for PyObject<'p> {
    #[inline]
    fn type_object(py: Python<'p>, _ : Option<&Self>) -> PyType<'p> {
        unsafe { PyType::from_type_ptr(py, &mut ffi::PyBaseObject_Type) }
    }
}

impl <'p> ToPythonPointer for PyObject<'p> {
    #[inline]
    fn as_ptr(&self) -> *mut ffi::PyObject {
        *self.ptr
    }
    
    #[inline]
    fn steal_ptr(self) -> *mut ffi::PyObject {
        let ptr = *self.ptr;
        unsafe { ::std::mem::forget(self); }
        ptr
    }
}


impl <'p> PyObject<'p> {
    /// Creates a PyObject instance for the given FFI pointer.
    /// This moves ownership over the pointer into the PyObject.
    /// Undefined behavior if the pointer is NULL or invalid.
    #[inline]
    pub unsafe fn from_owned_ptr(py : Python<'p>, ptr : *mut ffi::PyObject) -> PyObject<'p> {
        debug_assert!(!ptr.is_null() && ffi::Py_REFCNT(ptr) > 0);
        PyObject { py: py, ptr: NonZero::new(ptr) }
    }
    
    /// Creates a PyObject instance for the given FFI pointer.
    /// Calls Py_INCREF() on the ptr.
    /// Undefined behavior if the pointer is NULL or invalid.
    #[inline]
    pub unsafe fn from_borrowed_ptr(py : Python<'p>, ptr : *mut ffi::PyObject) -> PyObject<'p> {
        debug_assert!(!ptr.is_null() && ffi::Py_REFCNT(ptr) > 0);
        ffi::Py_INCREF(ptr);
        PyObject { py: py, ptr: NonZero::new(ptr) }
    }

    /// Creates a PyObject instance for the given FFI pointer.
    /// This moves ownership over the pointer into the PyObject.
    /// Returns None for null pointers; undefined behavior if the pointer is invalid.
    #[inline]
    pub unsafe fn from_owned_ptr_opt(py: Python<'p>, ptr: *mut ffi::PyObject) -> Option<PyObject<'p>> {
        if ptr.is_null() {
            None
        } else {
            Some(PyObject::from_owned_ptr(py, ptr))
        }
    }
    
    /// Returns None for null pointers; undefined behavior if the pointer is invalid.
    #[inline]
    pub unsafe fn from_borrowed_ptr_opt(py: Python<'p>, ptr: *mut ffi::PyObject) -> Option<PyObject<'p>> {
        if ptr.is_null() {
            None
        } else {
            Some(PyObject::from_borrowed_ptr(py, ptr))
        }
    }
    
    /// Transmutes an owned FFI pointer to &PyObject.
    /// Undefined behavior if the pointer is NULL or invalid.
    #[inline]
    pub unsafe fn borrow_from_owned_ptr<'a>(py : Python<'p>, ptr : &'a *mut ffi::PyObject) -> &'a PyObject<'p> {
        debug_assert!(!ptr.is_null() && ffi::Py_REFCNT(*ptr) > 0);
        transmute(ptr)
    }
    
    /// Transmutes a slice of owned FFI pointers to &[PyObject].
    /// Undefined behavior if the pointer is NULL or invalid.
    #[inline]
    pub unsafe fn borrow_from_owned_ptr_slice<'a>(py : Python<'p>, ptr : &'a [*mut ffi::PyObject]) -> &'a [PyObject<'p>] {
        transmute(ptr)
    }
    
    /// Retrieves the reference count of this python object.
    #[inline]
    pub fn get_refcnt(&self) -> usize {
        unsafe { ffi::Py_REFCNT(self.as_ptr()) as usize }
    }

    #[inline]
    pub fn get_type(&self) -> &PyType<'p> {
        unsafe {
            let t : &*mut ffi::PyTypeObject = &(*self.as_ptr()).ob_type;
            transmute(t)
        }
    }
    
    /// Casts the PyObject to a concrete python object type.
    /// Causes undefined behavior if the object is not of the expected type.
    /// This is a wrapper function around PythonObject::unchecked_downcast_from().
    #[inline]
    pub unsafe fn unchecked_cast_into<T>(self) -> T where T: PythonObject<'p> {
        PythonObject::unchecked_downcast_from(self)
    }
    
    /// Casts the PyObject to a concrete python object type.
    /// Returns a python TypeError if the object is not of the expected type.
    /// This is a wrapper function around PythonObjectWithCheckedDowncast::downcast_from().
    #[inline]
    pub fn cast_into<T>(self) -> Result<T, PythonObjectDowncastError<'p>> where T: PythonObjectWithCheckedDowncast<'p> {
        PythonObjectWithCheckedDowncast::downcast_from(self)
    }
    
    /// Casts the PyObject to a concrete python object type.
    /// Causes undefined behavior if the object is not of the expected type.
    /// This is a wrapper function around PythonObject::unchecked_downcast_borrow_from().
    #[inline]
    pub unsafe fn unchecked_cast_as<'s, T>(&'s self) -> &'s T where T: PythonObject<'p> {
        PythonObject::unchecked_downcast_borrow_from(self)
    }
    
    /// Casts the PyObject to a concrete python object type.
    /// Returns a python TypeError if the object is not of the expected type.
    /// This is a wrapper function around PythonObjectWithCheckedDowncast::downcast_borrow_from().
    #[inline]
    pub fn cast_as<'s, T>(&'s self) -> Result<&'s T, PythonObjectDowncastError<'p>> where T: PythonObjectWithCheckedDowncast<'p> {
        PythonObjectWithCheckedDowncast::downcast_borrow_from(self)
    }
    
    /// Extracts some type from the python object.
    /// This is a wrapper function around FromPyObject::from_py_object().
    #[inline]
    pub fn extract<'s, T>(&'s self) -> Result<T, PyErr<'p>> where T: ::conversion::FromPyObject<'p, 's> {
        ::conversion::FromPyObject::from_py_object(self)
    }
}

impl <'p> fmt::String for PyObject<'p> {
    fn fmt(&self, f : &mut fmt::Formatter) -> Result<(),  fmt::Error> {
        use objectprotocol::ObjectProtocol;
        let repr_obj = try!(self.str().map_err(|_| fmt::Error));
        let repr = try!(repr_obj.extract::<&str>().map_err(|_| fmt::Error));
        f.write_str(repr)
    }
}

impl <'p> fmt::Show for PyObject<'p> {
    fn fmt(&self, f : &mut fmt::Formatter) -> Result<(),  fmt::Error> {
        use objectprotocol::ObjectProtocol;
        let repr_obj = try!(self.repr().map_err(|_| fmt::Error));
        let repr = try!(repr_obj.extract::<&str>().map_err(|_| fmt::Error));
        f.write_str(repr)
    }
}

impl <'p> PartialEq for PyObject<'p> {
    #[inline]
    fn eq(&self, o : &PyObject<'p>) -> bool {
        self.ptr == o.ptr
    }
}
impl <'p> Eq for PyObject<'p> { }


#[test]
fn test_sizeof() {
    // should be a static_assert, but size_of is not a compile-time const
    // these are necessary for the transmutes in this module
    assert_eq!(size_of::<PyObject>(), size_of::<*mut ffi::PyObject>());
    assert_eq!(size_of::<PyType>(), size_of::<*mut ffi::PyTypeObject>());
}
