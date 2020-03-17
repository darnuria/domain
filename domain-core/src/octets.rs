//! Helper types and traits for dealing with generic octet sequences.

use core::{borrow, hash, fmt};
use core::cmp::Ordering;
use core::convert::TryFrom;
#[cfg(feature = "std")] use std::vec::Vec;
#[cfg(feature = "bytes")] use bytes::{Bytes, BytesMut};
#[cfg(feature = "smallvec")] use smallvec::{Array, SmallVec};
use derive_more::Display;
use crate::name::ToDname;
use crate::net::{Ipv4Addr, Ipv6Addr};


//------------ OctetsExt -----------------------------------------------------

pub trait OctetsExt: AsRef<[u8]> {
    fn truncate(&mut self, len: usize);
}

impl<'a> OctetsExt for &'a [u8] {
    fn truncate(&mut self, len: usize) {
        if len < self.len() {
            *self = &self[..len]
        }
    }
}

impl OctetsExt for Vec<u8> {
    fn truncate(&mut self, len: usize) {
        self.truncate(len)
    }
}

#[cfg(feature = "bytes")]
impl OctetsExt for Bytes {
    fn truncate(&mut self, len: usize) {
        self.truncate(len)
    }
}

#[cfg(feature = "smallvec")]
impl<A: Array<Item = u8>> OctetsExt for SmallVec<A> {
    fn truncate(&mut self, len: usize) {
        self.truncate(len)
    }
}


//------------ OctetsRef -----------------------------------------------------

pub trait OctetsRef: AsRef<[u8]> + Copy + Sized {
    type Range: AsRef<[u8]>;

    fn range(self, start: usize, end: usize) -> Self::Range;

    fn range_from(self, start: usize) -> Self::Range {
        self.range(start, self.as_ref().len())
    }

    fn range_to(self, end: usize) -> Self::Range {
        self.range(0, end)
    }

    fn range_all(self) -> Self::Range {
        self.range(0, self.as_ref().len())
    }
}

impl<'a, T: OctetsRef> OctetsRef for &'a T {
    type Range = T::Range;

    fn range(self, start: usize, end: usize) -> Self::Range {
        (*self).range(start, end)
    }
}

impl<'a> OctetsRef for &'a [u8] {
    type Range = &'a [u8];

    fn range(self, start: usize, end: usize) -> Self::Range {
        &self[start..end]
    }
}

impl<'a> OctetsRef for &'a Vec<u8> {
    type Range = &'a [u8];

    fn range(self, start: usize, end: usize) -> Self::Range {
        &self[start..end]
    }
}

#[cfg(feature = "bytes")]
impl<'a> OctetsRef for &'a Bytes  {
    type Range = Bytes;

    fn range(self, start: usize, end: usize) -> Self::Range {
        self.slice(start..end)
    }
}

#[cfg(feature = "smallvec")]
impl<'a, A: Array<Item = u8>> OctetsRef for &'a SmallVec<A> {
    type Range = &'a [u8];

    fn range(self, start: usize, end: usize) -> Self::Range {
        &self.as_slice()[start..end]
    }
}


//------------ OctetsBuilder -------------------------------------------------

pub trait OctetsBuilder: AsRef<[u8]> + AsMut<[u8]> + Sized {

    fn append_slice(&mut self, slice: &[u8]) -> Result<(), ShortBuf>;
    fn truncate(&mut self, len: usize);

    fn len(&self) -> usize {
        self.as_ref().len()
    }

    fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }

    fn append_all<F>(&mut self, op: F) -> Result<(), ShortBuf>
    where F: FnOnce(&mut Self) -> Result<(), ShortBuf> {
        let pos = self.len();
        match op(self) {
            Ok(_) => Ok(()),
            Err(_) => {
                self.truncate(pos);
                Err(ShortBuf)
            }
        }
    }

    fn append_compressed_dname<N: ToDname>(
        &mut self,
        name: &N
    ) -> Result<(), ShortBuf> {
        if let Some(slice) = name.as_flat_slice() {
            self.append_slice(slice)
        }
        else {
            self.append_all(|target| {
                for label in name.iter_labels() {
                    label.build(target)?;
                }
                Ok(())
            })
        }
    }

    fn len_prefixed<F>(&mut self, op: F) -> Result<(), ShortBuf>
    where F: FnOnce(&mut Self) -> Result<(), ShortBuf> {
        let pos = self.len();
        self.append_slice(&[0; 2])?;
        match op(self) {
            Ok(_) => {
                let len = self.len() - pos - 2;
                if len > usize::from(u16::max_value()) {
                    self.truncate(pos);
                    Err(ShortBuf)
                }
                else {
                    self.as_mut()[pos..pos + 2].copy_from_slice(
                        &(len as u16).to_be_bytes()
                    );
                    Ok(())
                }
            }
            Err(_) => {
                self.truncate(pos);
                Err(ShortBuf)
            }
        }
    }
}

#[cfg(feature = "std")]
impl OctetsBuilder for Vec<u8> {
    fn append_slice(&mut self, slice: &[u8]) -> Result<(), ShortBuf> {
        self.extend_from_slice(slice);
        Ok(())
    }

    fn truncate(&mut self, len: usize) {
        Vec::truncate(self, len)
    }
}

#[cfg(feature="bytes")]
impl OctetsBuilder for BytesMut {
    fn append_slice(&mut self, slice: &[u8]) -> Result<(), ShortBuf> {
        self.extend_from_slice(slice);
        Ok(())
    }

    fn truncate(&mut self, len: usize) {
        BytesMut::truncate(self, len)
    }

}

#[cfg(feature = "smallvec")]
impl<A: Array<Item = u8>> OctetsBuilder for SmallVec<A> {
    fn append_slice(&mut self, slice: &[u8]) -> Result<(), ShortBuf> {
        self.extend_from_slice(slice);
        Ok(())
    }

    fn truncate(&mut self, len: usize) {
        SmallVec::truncate(self, len)
    }
}


//------------ EmptyBuilder --------------------------------------------------

pub trait EmptyBuilder {
    fn empty() -> Self;

    fn with_capacity(capacity: usize) -> Self;
}

#[cfg(feature = "std")]
impl EmptyBuilder for Vec<u8> {
    fn empty() -> Self {
        Vec::new()
    }

    fn with_capacity(capacity: usize) -> Self {
        Vec::with_capacity(capacity)
    }
}

#[cfg(feature="bytes")]
impl EmptyBuilder for BytesMut {
    fn empty() -> Self {
        BytesMut::new()
    }

    fn with_capacity(capacity: usize) -> Self {
        BytesMut::with_capacity(capacity)
    }
}

#[cfg(feature = "smallvec")]
impl<A: Array<Item = u8>> EmptyBuilder for SmallVec<A> {
    fn empty() -> Self {
        SmallVec::new()
    }

    fn with_capacity(capacity: usize) -> Self {
        SmallVec::with_capacity(capacity)
    }
}


//------------ IntoOctets ----------------------------------------------------

pub trait IntoOctets {
    type Octets: AsRef<[u8]>;

    fn into_octets(self) -> Self::Octets;
}

#[cfg(feature = "std")]
impl IntoOctets for Vec<u8> {
    type Octets = Self;

    fn into_octets(self) -> Self::Octets {
        self
    }
}

#[cfg(feature="bytes")]
impl IntoOctets for BytesMut {
    type Octets = Bytes;

    fn into_octets(self) -> Self::Octets {
        self.freeze()
    }
}

#[cfg(feature = "smallvec")]
impl<A: Array<Item = u8>> IntoOctets for SmallVec<A> {
    type Octets = Self;

    fn into_octets(self) -> Self::Octets {
        self
    }
}


//------------ IntoBuilder ---------------------------------------------------

pub trait IntoBuilder {
    type Builder: OctetsBuilder;

    fn into_builder(self) -> Self::Builder;
}

#[cfg(feature = "std")]
impl IntoBuilder for Vec<u8> {
    type Builder = Self;

    fn into_builder(self) -> Self::Builder {
        self
    }
}

#[cfg(feature = "std")]
impl<'a> IntoBuilder for &'a [u8] {
    type Builder = Vec<u8>;

    fn into_builder(self) -> Self::Builder {
        self.into()
    }
}

#[cfg(feature="bytes")]
impl IntoBuilder for Bytes {
    type Builder = BytesMut;

    fn into_builder(self) -> Self::Builder {
        // XXX Currently, we need to copy to do this. If bytes gains a way
        //     to convert from Bytes to BytesMut for non-shared data without
        //     copying, we should change this.
        BytesMut::from(self.as_ref())
    }
}

#[cfg(feature = "smallvec")]
impl<A: Array<Item = u8>> IntoBuilder for SmallVec<A> {
    type Builder = Self;

    fn into_builder(self) -> Self::Builder {
        self
    }
}


//------------ FromBuilder ---------------------------------------------------

pub trait FromBuilder: AsRef<[u8]> + Sized {
    type Builder: OctetsBuilder + IntoOctets<Octets = Self>;

    fn from_builder(builder: Self::Builder) -> Self;
}

#[cfg(feature = "std")]
impl FromBuilder for Vec<u8> {
    type Builder = Self;

    fn from_builder(builder: Self) -> Self {
        builder
    }
}

#[cfg(feature="bytes")]
impl FromBuilder for Bytes {
    type Builder = BytesMut;

    fn from_builder(builder: Self::Builder) -> Self {
        builder.freeze()
    }
}

#[cfg(feature = "smallvec")]
impl<A: Array<Item = u8>> FromBuilder for SmallVec<A> {
    type Builder = Self;

    fn from_builder(builder: Self) -> Self {
        builder
    }
}


//------------ Compose -------------------------------------------------------

/// A type that knows how to compose itself.
///
/// The term ‘composing’ refers to the process of creating a DNS wire-format
/// representation of a value’s data.
pub trait Compose {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf>;

    fn compose_canonical<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        self.compose(target)
    }
}

impl<'a, C: Compose + ?Sized> Compose for &'a C {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        (*self).compose(target)
    }

    fn compose_canonical<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        (*self).compose_canonical(target)
    }
}

impl Compose for i8 {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_slice(&[*self as u8])
    }
}

impl Compose for u8 {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_slice(&[*self])
    }
}

impl Compose for i16 {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_slice(&self.to_be_bytes())
    }
}

impl Compose for u16 {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_slice(&self.to_be_bytes())
    }
}

impl Compose for i32 {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_slice(&self.to_be_bytes())
    }
}

impl Compose for u32 {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_slice(&self.to_be_bytes())
    }
}

impl Compose for Ipv4Addr {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_slice(&self.octets())
    }
}

impl Compose for Ipv6Addr {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_slice(&self.octets())
    }
}

//------------ octets_array --------------------------------------------------

#[macro_export]
macro_rules! octets_array {
    ( $vis:vis $name:ident => $len:expr) => {
        #[derive(Clone)]
        $vis struct $name {
            octets: [u8; $len],
            len: usize
        }

        impl $name {
            pub fn new() -> Self {
                Default::default()
            }

            pub fn as_slice(&self) -> &[u8] {
                &self.octets[..self.len]
            }

            pub fn as_slice_mut(&mut self) -> &mut [u8] {
                &mut self.octets[..self.len]
            }
        }

        impl Default for $name {
            fn default() -> Self {
                $name {
                    octets: [0; $len],
                    len: 0
                }
            }
        }

        impl<'a> TryFrom<&'a [u8]> for $name {
            type Error = ShortBuf;

            fn try_from(src: &'a [u8]) -> Result<Self, ShortBuf> {
                let len = src.len();
                if len > $len {
                    Err(ShortBuf)
                }
                else {
                    let mut res = Self::default();
                    res.octets[..len].copy_from_slice(src);
                    res.len = len;
                    Ok(res)
                }
            }
        }

        impl core::ops::Deref for $name {
            type Target = [u8];

            fn deref(&self) -> &[u8] {
                self.as_slice()
            }
        }

        impl core::ops::DerefMut for $name {
            fn deref_mut(&mut self) -> &mut [u8] {
                self.as_slice_mut()
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                self.as_slice()
            }
        }

        impl AsMut<[u8]> for $name {
            fn as_mut(&mut self) -> &mut [u8] {
                self.as_slice_mut()
            }
        }

        impl borrow::Borrow<[u8]> for $name {
            fn borrow(&self) -> &[u8] {
                self.as_slice()
            }
        }

        impl borrow::BorrowMut<[u8]> for $name {
            fn borrow_mut(&mut self) -> &mut [u8] {
                self.as_slice_mut()
            }
        }

        impl $crate::octets::OctetsBuilder for $name {
            fn append_slice(&mut self, slice: &[u8]) -> Result<(), ShortBuf> {
                if slice.len() > $len - self.len {
                    Err(ShortBuf)
                }
                else {
                    let end = self.len + slice.len();
                    self.octets[self.len..end].copy_from_slice(slice);
                    self.len = end;
                    Ok(())
                }
            }

            fn truncate(&mut self, len: usize) {
                if len < self.len {
                    self.len = len
                }
            }
        }

        impl $crate::octets::EmptyBuilder for $name {
            fn empty() -> Self {
                $name {
                    octets: [0; $len],
                    len: 0
                }
            }

            fn with_capacity(_capacity: usize) -> Self {
                Self::empty()
            }
        }

        impl $crate::octets::IntoBuilder for $name {
            type Builder = Self;

            fn into_builder(self) -> Self::Builder {
                self
            }
        }

        impl $crate::octets::FromBuilder for $name {
            type Builder = Self;

            fn from_builder(builder: Self::Builder) -> Self {
                builder
            }
        }

        impl $crate::octets::IntoOctets for $name {
            type Octets = Self;

            fn into_octets(self) -> Self::Octets {
                self
            }
        }

        impl<T: AsRef<[u8]>> PartialEq<T> for $name {
            fn eq(&self, other: &T) -> bool {
                self.as_slice().eq(other.as_ref())
            }
        }

        impl Eq for $name { }

        impl<T: AsRef<[u8]>> PartialOrd<T> for $name {
            fn partial_cmp(&self, other: &T) -> Option<Ordering> {
                self.as_slice().partial_cmp(other.as_ref())
            }
        }

        impl Ord for $name {
            fn cmp(&self, other: &Self) -> Ordering {
                self.as_slice().cmp(other.as_slice())
            }
        }

        impl hash::Hash for $name {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.as_slice().hash(state)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&self.as_slice())
                    .finish()
            }
        }
    }
}

octets_array!(pub Octets32 => 32);
octets_array!(pub Octets64 => 64);
octets_array!(pub Octets128 => 128);
octets_array!(pub Octets256 => 256);
octets_array!(pub Octets512 => 512);
octets_array!(pub Octets1024 => 1024);
octets_array!(pub Octets2048 => 2048);
octets_array!(pub Octets4096 => 4096);


#[cfg(feature = "smallvec")]
pub type OctetsVec = SmallVec<[u8; 24]>;

//------------ ShortBuf ------------------------------------------------------

/// An attempt was made to go beyond the end of a buffer.
#[derive(Clone, Debug, Display, Eq, PartialEq)]
#[display(fmt="unexpected end of buffer")]
pub struct ShortBuf;

#[cfg(feature = "std")]
impl std::error::Error for ShortBuf { }

