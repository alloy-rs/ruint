//! Support for the [`pyo3`](https://crates.io/crates/pyo3) crate.
//!
//! Conversion is to/from Python native integers. Beware that Python native
//! integers are unbounded and not a ring modulo a power of two like [`Uint`].
//!
//! This uses the not-so-public `_PyLong_FromByteArray`, `_PyLong_AsByteArray`
//! ABI (`PyLong_AsNativeBytes` on Python 3.13 and later), which according to
//! this [Stackoverflow answer][so] is the accepted way to do efficient bigint
//! conversions. It is supported by [CPython][cpython] and [PyPy][pypy].
//!
//! [so]: https://stackoverflow.com/a/18326068
//! [cpython]: https://github.com/python/cpython/blob/e8165d47b852e933c176209ddc0b5836a9b0d5f4/Include/cpython/longobject.h#L47
//! [pypy]: https://foss.heptapod.net/pypy/pypy/-/blob/branch/default/pypy/module/cpyext/longobject.py#L238
//!
//! The implementation uses Pyo3 builtin `u64` conversion when $\mathtt{BITS} ≤
//! 64$ and otherwise uses similar conversion to Pyo3 builtin `num-bigint`
//! support. See Pyo3's [`num.rs`][num] and [`num_bigint.rs`][bigint] for
//! reference.
//!
//! [num]: https://github.com/PyO3/pyo3/blob/caaf7bbda74f873297d277733c157338f5492580/src/types/num.rs#L81
//! [bigint]: https://github.com/PyO3/pyo3/blob/4a68273b173ef86dac059106cc0b5b3c2c9830e2/src/conversions/num_bigint.rs#L80

#![cfg(feature = "pyo3")]
#![cfg_attr(docsrs, doc(cfg(feature = "pyo3")))]

use crate::Uint;
use pyo3::{
    Bound, FromPyObject, IntoPyObject, PyAny, PyErr, PyResult, Python,
    exceptions::PyOverflowError,
    ffi,
    types::{PyAnyMethods, PyInt},
};

// This implementation via &Self mirrors the implementations for biguint in
// pyo3.
impl<'a, const BITS: usize, const LIMBS: usize> IntoPyObject<'a> for Uint<BITS, LIMBS> {
    type Target = PyInt;

    type Output = Bound<'a, Self::Target>;

    type Error = PyErr;

    fn into_pyobject(self, py: Python<'a>) -> Result<Self::Output, Self::Error> {
        (&self).into_pyobject(py)
    }
}

impl<'a, const BITS: usize, const LIMBS: usize> IntoPyObject<'a> for &Uint<BITS, LIMBS> {
    type Target = PyInt;
    type Output = Bound<'a, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'a>) -> Result<Self::Output, Self::Error> {
        // Fast path for small ints
        if BITS <= 64 {
            let value = self.as_limbs().first().copied().unwrap_or(0);
            return Ok(value.into_pyobject(py).unwrap());
        }

        // Convert using little endian bytes (trivial on LE archs)
        // and `_PyLong_FromByteArray`.
        let bytes = self.as_le_bytes();

        unsafe {
            let obj =
                ffi::_PyLong_FromByteArray(bytes.as_ptr().cast(), bytes.len(), 1, false.into());
            let bound = Bound::from_owned_ptr(py, obj);
            Ok(bound.downcast_into_unchecked())
        }
    }
}

impl<'a, const BITS: usize, const LIMBS: usize> FromPyObject<'a> for Uint<BITS, LIMBS> {
    fn extract_bound(ob: &Bound<'a, PyAny>) -> PyResult<Self> {
        let py = ob.py();

        // Raises `TypeError` for anything that is not an `int` or has no
        // `__index__`.
        let num = unsafe { Bound::from_owned_ptr_or_err(py, ffi::PyNumber_Index(ob.as_ptr()))? };

        let mut result = Self::ZERO;

        // On little endian let Python write directly to the uint.
        #[cfg(target_endian = "little")]
        {
            write_le_bytes(&num, unsafe { result.as_le_slice_mut() })?;

            // Check mask since we wrote raw.
            if let Some(last) = result.as_limbs().last()
                && *last > Self::MASK
            {
                return Err(overflow_error::<BITS>());
            }
        }

        // On big endian we use an intermediate
        #[cfg(not(target_endian = "little"))]
        {
            let mut raw = vec![0_u8; LIMBS * 8];
            write_le_bytes(&num, &mut raw)?;
            result = Self::try_from_le_slice(raw.as_slice()).ok_or_else(overflow_error::<BITS>)?;
        }

        Ok(result)
    }
}

fn overflow_error<const BITS: usize>() -> PyErr {
    PyOverflowError::new_err(format!("Number to large to fit Uint<{BITS}>"))
}

/// Writes the unsigned value of the Python `int` `num` to `buf` as little
/// endian bytes.
///
/// Raises `OverflowError` if the value is negative or does not fit `buf`.
fn write_le_bytes(num: &Bound<'_, PyAny>, buf: &mut [u8]) -> PyResult<()> {
    // `_PyLong_AsByteArray` has an extra parameter since Python 3.13, which
    // `pyo3-ffi` does not declare.
    #[cfg(Py_3_13)]
    {
        let flags = ffi::Py_ASNATIVEBYTES_LITTLE_ENDIAN
            | ffi::Py_ASNATIVEBYTES_UNSIGNED_BUFFER
            | ffi::Py_ASNATIVEBYTES_REJECT_NEGATIVE;

        // Zero needs one byte, so an empty `buf` needs a scratch byte to hold
        // it.
        let mut scratch = [0_u8; 1];
        let empty = buf.is_empty();
        let out = if empty { &mut scratch[..] } else { &mut *buf };

        let cap = out.len();
        let len = unsafe {
            ffi::PyLong_AsNativeBytes(
                num.as_ptr(),
                out.as_mut_ptr().cast(),
                cap.try_into().unwrap(),
                flags,
            )
        };

        // `len` is the size needed to hold the value.
        match usize::try_from(len) {
            Ok(len) if len <= cap && !(empty && scratch[0] != 0) => Ok(()),
            Ok(_) => Err(PyOverflowError::new_err("int too big to convert")),
            Err(_) => {
                // Only a negative value raises `ValueError`.
                let err = PyErr::fetch(num.py());
                Err(
                    if err.is_instance_of::<pyo3::exceptions::PyValueError>(num.py()) {
                        PyOverflowError::new_err("can't convert negative int to unsigned")
                    } else {
                        err
                    },
                )
            }
        }
    }

    #[cfg(not(Py_3_13))]
    {
        let status = unsafe {
            ffi::_PyLong_AsByteArray(num.as_ptr().cast(), buf.as_mut_ptr(), buf.len(), 1, 0)
        };
        if status != 0 {
            return Err(PyErr::fetch(num.py()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        aliases::{U0, U8, U64, U256, U512},
        const_for, nlimbs,
    };
    use proptest::proptest;
    use pyo3::{
        PyTypeInfo,
        exceptions::PyTypeError,
        types::{PyBytes, PyList, PyString},
    };

    fn err_is<T: for<'a> FromPyObject<'a> + core::fmt::Debug, E: PyTypeInfo>(
        obj: &Bound<'_, PyAny>,
    ) -> bool {
        T::extract_bound(obj)
            .unwrap_err()
            .is_instance_of::<E>(obj.py())
    }

    #[test]
    fn test_roundtrip() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            const_for!(BITS in SIZES {
                const LIMBS: usize = nlimbs(BITS);
                type U = Uint<BITS, LIMBS>;
                proptest!(|(value: U)| {
                    let obj = value.into_pyobject(py).unwrap();
                    let native = Uint::<BITS, LIMBS>::extract_bound(&obj).unwrap();
                    assert_eq!(value, native);
                });
            });
        });
    }

    #[test]
    fn test_errors() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py: Python<'_>| {
            let obj = (-1_i64).into_pyobject(py).unwrap();
            assert!(err_is::<U0, PyOverflowError>(&obj));
            assert!(err_is::<U256, PyOverflowError>(&obj));

            let obj = (-(1_i128 << 100)).into_pyobject(py).unwrap();
            assert!(err_is::<U256, PyOverflowError>(&obj));

            let obj = (1000_i64).into_pyobject(py).unwrap();
            assert!(err_is::<U0, PyOverflowError>(&obj));
            assert!(err_is::<U8, PyOverflowError>(&obj));

            let obj = U512::MAX.into_pyobject(py).unwrap();
            assert!(err_is::<U0, PyOverflowError>(&obj));
            assert!(err_is::<U64, PyOverflowError>(&obj));
            assert!(err_is::<U256, PyOverflowError>(&obj));

            let obj = (U512::ONE << 256_usize).into_pyobject(py).unwrap();
            assert!(err_is::<U256, PyOverflowError>(&obj));
        });
    }

    #[test]
    fn test_non_int() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let objs = [
                PyString::new(py, "100").into_any(),
                py.None().into_bound(py),
                1.5_f64.into_pyobject(py).unwrap().into_any(),
                PyList::new(py, [0_u8; 64]).unwrap().into_any(),
                PyBytes::new(py, &[1]).into_any(),
            ];
            for obj in &objs {
                assert!(err_is::<U0, PyTypeError>(obj));
                assert!(err_is::<U64, PyTypeError>(obj));
                assert!(err_is::<U256, PyTypeError>(obj));
            }
        });
    }
}
