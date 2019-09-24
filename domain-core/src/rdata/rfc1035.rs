//! Record data from [RFC 1035].
//!
//! This RFC defines the initial set of record types.
//!
//! [RFC 1035]: https://tools.ietf.org/html/rfc1035

use core::{hash, fmt, ops};
use core::cmp::Ordering;
use core::str::FromStr;
#[cfg(feature="bytes")] use bytes::{Bytes, BytesMut};
use unwrap::unwrap;
use crate::cmp::CanonicalOrd;
use crate::iana::Rtype;
use crate::charstr::{CharStr, PushError};
#[cfg(feature="bytes")] use crate::master::scan::{
    CharSource, ScanError, Scan, Scanner, SyntaxError
};
use crate::str::Symbol;
use crate::name::{ParsedDname, ToDname};
use crate::net::Ipv4Addr;
use crate::octets::{
    Compose, EmptyBuilder, FromBuilder, IntoOctets, OctetsBuilder,
    ParseOctets, ShortBuf
};
use crate::parse::{ParseAll, ParseAllError, ParseOpenError, Parse, Parser};
use crate::serial::Serial;
use super::RtypeRecordData;


//------------ dname_type! --------------------------------------------------

/// A macro for implementing a record data type with a single domain name.
///
/// Implements some basic methods plus the `RecordData`, `FlatRecordData`,
/// and `Display` traits.
macro_rules! dname_type {
    ($(#[$attr:meta])* ( $target:ident, $rtype:ident, $field:ident ) ) => {
        $(#[$attr])*
        #[derive(Clone, Debug)]
        pub struct $target<N> {
            $field: N
        }

        impl<N> $target<N> {
            pub fn new($field: N) -> Self {
                $target { $field: $field }
            }

            pub fn $field(&self) -> &N {
                &self.$field
            }
        }

        //--- From and FromStr

        impl<N> From<N> for $target<N> {
            fn from(name: N) -> Self {
                Self::new(name)
            }
        }

        impl<N: FromStr> FromStr for $target<N> {
            type Err = N::Err;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                N::from_str(s).map(Self::new)
            }
        }


        //--- PartialEq and Eq

        impl<N, NN> PartialEq<$target<NN>> for $target<N>
        where N: ToDname, NN: ToDname {
            fn eq(&self, other: &$target<NN>) -> bool {
                self.$field.name_eq(&other.$field)
            }
        }

        impl<N: ToDname> Eq for $target<N> { }


        //--- PartialOrd, Ord, and CanonicalOrd

        impl<N, NN> PartialOrd<$target<NN>> for $target<N>
        where N: ToDname, NN: ToDname {
            fn partial_cmp(&self, other: &$target<NN>) -> Option<Ordering> {
                Some(self.$field.name_cmp(&other.$field))
            }
        }

        impl<N: ToDname> Ord for $target<N> {
            fn cmp(&self, other: &Self) -> Ordering {
                self.$field.name_cmp(&other.$field)
            }
        }

        impl<N: ToDname, NN: ToDname> CanonicalOrd<$target<NN>> for $target<N> {
            fn canonical_cmp(&self, other: &$target<NN>) -> Ordering {
                self.$field.lowercase_composed_cmp(&other.$field)
            }
        }


        //--- Hash

        impl<N: hash::Hash> hash::Hash for $target<N> {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.$field.hash(state)
            }
        }


        //--- Parse, ParseAll, and Compose

        impl<Octets> Parse<Octets> for $target<ParsedDname<Octets>>
        where Octets: ParseOctets {
            type Err = <ParsedDname<Octets> as Parse<Octets>>::Err;

            fn parse(parser: &mut Parser<Octets>) -> Result<Self, Self::Err> {
                ParsedDname::parse(parser).map(Self::new)
            }

            fn skip(parser: &mut Parser<Octets>) -> Result<(), Self::Err> {
                ParsedDname::skip(parser).map_err(Into::into)
            }
        }

        impl<Octets> ParseAll<Octets> for $target<ParsedDname<Octets>>
        where Octets: ParseOctets {
            type Err = <ParsedDname<Octets> as ParseAll<Octets>>::Err;

            fn parse_all(
                parser: &mut Parser<Octets>,
                len: usize
            ) -> Result<Self, Self::Err> {
                ParsedDname::parse_all(parser, len).map(Self::new)
            }
        }

        impl<N: ToDname> Compose for $target<N> {
            fn compose<T: OctetsBuilder>(
                &self,
                target: &mut T
            ) -> Result<(), ShortBuf> {
                target.append_compressed_dname(&self.$field)
            }

            fn compose_canonical<T: OctetsBuilder>(
                &self,
                target: &mut T
            ) -> Result<(), ShortBuf> {
                self.$field.compose_canonical(target)
            }
        }


        //--- Scan and Display

        #[cfg(feature="bytes")] 
        impl<N: Scan> Scan for $target<N> {
            fn scan<C: CharSource>(scanner: &mut Scanner<C>)
                                   -> Result<Self, ScanError> {
                N::scan(scanner).map(Self::new)
            }
        }

        impl<N: fmt::Display> fmt::Display for $target<N> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}.", self.$field)
            }
        }


        //--- RtypeRecordData

        impl<N> RtypeRecordData for $target<N> {
            const RTYPE: Rtype = Rtype::$rtype;
        }


        //--- Deref

        impl<N> ops::Deref for $target<N> {
            type Target = N;

            fn deref(&self) -> &Self::Target {
                &self.$field
            }
        }
    }
}


//------------ A ------------------------------------------------------------

/// A record data.
///
/// A records convey the IPv4 address of a host. The wire format is the 32
/// bit IPv4 address in network byte order. The master file format is the
/// usual dotted notation.
///
/// The A record type is defined in RFC 1035, section 3.4.1.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct A {
    addr: Ipv4Addr,
}

impl A {
    /// Creates a new A record data from an IPv4 address.
    pub fn new(addr: Ipv4Addr) -> A {
        A { addr }
    }

    /// Creates a new A record from the IPv4 address components.
    pub fn from_octets(a: u8, b: u8, c: u8, d: u8) -> A {
        A::new(Ipv4Addr::new(a, b, c, d))
    }

    pub fn addr(&self) -> Ipv4Addr { self.addr }
    pub fn set_addr(&mut self, addr: Ipv4Addr) { self.addr = addr }
}


//--- From and FromStr

impl From<Ipv4Addr> for A {
    fn from(addr: Ipv4Addr) -> Self {
        Self::new(addr)
    }
}

impl From<A> for Ipv4Addr {
    fn from(a: A) -> Self {
        a.addr
    }
}

#[cfg(feature = "std")]
impl FromStr for A {
    type Err = <Ipv4Addr as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ipv4Addr::from_str(s).map(A::new)
    }
}


//--- CanonicalOrd

impl CanonicalOrd for A {
    fn canonical_cmp(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }
}


//--- Parse, ParseAll and Compose

impl<Octets: AsRef<[u8]>> Parse<Octets> for A {
    type Err = <Ipv4Addr as Parse<Octets>>::Err;

    fn parse(parser: &mut Parser<Octets>) -> Result<Self, Self::Err> {
        Ipv4Addr::parse(parser).map(Self::new)
    }

    fn skip(parser: &mut Parser<Octets>) -> Result<(), Self::Err> {
        Ipv4Addr::skip(parser)
    }
}

impl<Octets: AsRef<[u8]>> ParseAll<Octets> for A {
    type Err = <Ipv4Addr as ParseAll<Octets>>::Err;

    fn parse_all(
        parser: &mut Parser<Octets>,
        len: usize
    ) -> Result<Self, Self::Err> {
        Ipv4Addr::parse_all(parser, len).map(Self::new)
    }
}

impl Compose for A {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        self.addr.compose(target)
    }
}


//--- Scan and Display

#[cfg(feature="bytes")] 
impl Scan for A {
    fn scan<C: CharSource>(scanner: &mut Scanner<C>)
                           -> Result<Self, ScanError> {
        scanner.scan_string_phrase(|res| A::from_str(&res).map_err(Into::into))
    }
}

impl fmt::Display for A {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.addr.fmt(f)
    }
}


//--- RtypeRecordData

impl RtypeRecordData for A {
    const RTYPE: Rtype = Rtype::A;
}


//--- Deref and DerefMut

impl ops::Deref for A {
    type Target = Ipv4Addr;

    fn deref(&self) -> &Self::Target {
        &self.addr
    }
}

impl ops::DerefMut for A {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.addr
    }
}


//--- AsRef and AsMut

impl AsRef<Ipv4Addr> for A {
    fn as_ref(&self) -> &Ipv4Addr {
        &self.addr
    }
}

impl AsMut<Ipv4Addr> for A {
    fn as_mut(&mut self) -> &mut Ipv4Addr {
        &mut self.addr
    }
}


//------------ Cname --------------------------------------------------------

dname_type! {
    /// CNAME record data.
    ///
    /// The CNAME record specifies the canonical or primary name for domain
    /// name alias.
    ///
    /// The CNAME type is defined in RFC 1035, section 3.3.1.
    (Cname, Cname, cname)
}


//------------ Hinfo --------------------------------------------------------

/// Hinfo record data.
///
/// Hinfo records are used to acquire general information about a host,
/// specifically the CPU type and operating system type.
///
/// The Hinfo type is defined in RFC 1035, section 3.3.2.
#[derive(Clone)]
pub struct Hinfo<Octets> {
    cpu: CharStr<Octets>,
    os: CharStr<Octets>,
}

impl<Octets> Hinfo<Octets> {
    /// Creates a new Hinfo record data from the components.
    pub fn new(cpu: CharStr<Octets>, os: CharStr<Octets>) -> Self {
        Hinfo { cpu, os }
    }

    /// The CPU type of the host.
    pub fn cpu(&self) -> &CharStr<Octets> {
        &self.cpu
    }

    /// The operating system type of the host.
    pub fn os(&self) -> &CharStr<Octets> {
        &self.os
    }
}


//--- PartialEq and Eq

impl<Octets, Other> PartialEq<Hinfo<Other>> for Hinfo<Octets>
where Octets: AsRef<[u8]>, Other: AsRef<[u8]> {
    fn eq(&self, other: &Hinfo<Other>) -> bool {
        self.cpu.eq(&other.cpu) && self.os.eq(&other.os)
    }
}

impl<Octets: AsRef<[u8]>> Eq for Hinfo<Octets> { }


//--- PartialOrd, CanonicalOrd, and Ord

impl<Octets, Other> PartialOrd<Hinfo<Other>> for Hinfo<Octets>
where Octets: AsRef<[u8]>, Other: AsRef<[u8]> {
    fn partial_cmp(&self, other: &Hinfo<Other>) -> Option<Ordering> {
        match self.cpu.partial_cmp(&other.cpu) {
            Some(Ordering::Equal) => { }
            other => return other
        }
        self.os.partial_cmp(&other.os)
    }
}

impl<Octets, Other> CanonicalOrd<Hinfo<Other>> for Hinfo<Octets>
where Octets: AsRef<[u8]>, Other: AsRef<[u8]> {
    fn canonical_cmp(&self, other: &Hinfo<Other>) -> Ordering {
        match self.cpu.canonical_cmp(&other.cpu) {
            Ordering::Equal => { }
            other => return other
        }
        self.os.canonical_cmp(&other.os)
    }
}

impl<Octets: AsRef<[u8]>> Ord for Hinfo<Octets> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.cpu.cmp(&other.cpu) {
            Ordering::Equal => { }
            other => return other
        }
        self.os.cmp(&other.os)
    }
}


//--- Hash

impl<Octets: AsRef<[u8]>> hash::Hash for Hinfo<Octets> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.cpu.hash(state);
        self.os.hash(state);
    }
}


//--- Parse, Compose, and Compress

impl<Octets: ParseOctets> Parse<Octets> for Hinfo<Octets> {
    type Err = ShortBuf;

    fn parse(parser: &mut Parser<Octets>) -> Result<Self, Self::Err> {
        Ok(Self::new(CharStr::parse(parser)?, CharStr::parse(parser)?))
    }

    fn skip(parser: &mut Parser<Octets>) -> Result<(), Self::Err> {
        CharStr::skip(parser)?;
        CharStr::skip(parser)?;
        Ok(())
    }
}

impl<Octets: ParseOctets> ParseAll<Octets> for Hinfo<Octets> {
    type Err = ParseAllError;

    fn parse_all(
        parser: &mut Parser<Octets>,
        len: usize
    ) -> Result<Self, Self::Err> {
        let cpu = CharStr::parse(parser)?;
        let len = match len.checked_sub(cpu.len() + 1) {
            Some(len) => len,
            None => return Err(ParseAllError::ShortField)
        };
        let os = CharStr::parse_all(parser, len)?;
        Ok(Hinfo::new(cpu, os))
    }
}

impl<Octets: AsRef<[u8]>> Compose for Hinfo<Octets> {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_all(|target| {
            self.cpu.compose(target)?;
            self.os.compose(target)
        })
    }
}


//--- Scan and Display

#[cfg(feature="bytes")] 
impl Scan for Hinfo<Bytes> {
    fn scan<C: CharSource>(scanner: &mut Scanner<C>)
                           -> Result<Self, ScanError> {
        Ok(Self::new(CharStr::scan(scanner)?, CharStr::scan(scanner)?))
    }
}

impl<Octets: AsRef<[u8]>> fmt::Display for Hinfo<Octets> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.cpu, self.os)
    }
}


//--- Debug

impl<Octets: AsRef<[u8]>> fmt::Debug for Hinfo<Octets> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Hinfo")
            .field("cpu", &self.cpu)
            .field("os", &self.os)
            .finish()
    }
}


//--- RtypeRecordData

impl<Octets> RtypeRecordData for Hinfo<Octets> {
    const RTYPE: Rtype = Rtype::Hinfo;
}


//------------ Mb -----------------------------------------------------------

dname_type! {
    /// MB record data.
    ///
    /// The experimental MB record specifies a host that serves a mailbox.
    ///
    /// The MB record type is defined in RFC 1035, section 3.3.3.
    (Mb, Mb, madname)
}


//------------ Md -----------------------------------------------------------

dname_type! {
    /// MD record data.
    ///
    /// The MD record specifices a host which has a mail agent for
    /// the domain which should be able to deliver mail for the domain.
    /// 
    /// The MD record is obsolete. It is recommended to either reject the record
    /// or convert them into an Mx record at preference 0.
    ///
    /// The MD record type is defined in RFC 1035, section 3.3.4.
    (Md, Md, madname)
}


//------------ Mf -----------------------------------------------------------

dname_type! {
    /// MF record data.
    ///
    /// The MF record specifices a host which has a mail agent for
    /// the domain which will be accept mail for forwarding to the domain.
    /// 
    /// The MF record is obsolete. It is recommended to either reject the record
    /// or convert them into an Mx record at preference 10.
    ///
    /// The MF record type is defined in RFC 1035, section 3.3.5.
    (Mf, Mf, madname)
}


//------------ Mg -----------------------------------------------------------

dname_type! {
    /// MG record data.
    ///
    /// The MG record specifices a mailbox which is a member of the mail group
    /// specified by the domain name.
    /// 
    /// The MG record is experimental.
    ///
    /// The MG record type is defined in RFC 1035, section 3.3.6.
    (Mg, Mg, madname)
}


//------------ Minfo --------------------------------------------------------

/// Minfo record data.
///
/// The Minfo record specifies a mailbox which is responsible for the mailing
/// list or mailbox and a mailbox that receives error messages related to the
/// list or box.
///
/// The Minfo record is experimental.
///
/// The Minfo record type is defined in RFC 1035, section 3.3.7.
#[derive(Clone, Debug, Hash)]
pub struct Minfo<N> {
    rmailbx: N,
    emailbx: N,
}

impl<N> Minfo<N> {
    /// Creates a new Minfo record data from the components.
    pub fn new(rmailbx: N, emailbx: N) -> Self {
        Minfo { rmailbx, emailbx }
    }

    /// The responsible mail box.
    ///
    /// The domain name specifies the mailbox which is responsible for the
    /// mailing list or mailbox. If this domain name is the root, the owner
    /// of the Minfo record is responsible for itself.
    pub fn rmailbx(&self) -> &N {
        &self.rmailbx
    }

    /// The error mail box.
    ///
    /// The domain name specifies a mailbox which is to receive error
    /// messages related to the mailing list or mailbox specified by the
    /// owner of the record. If this is the root domain name, errors should
    /// be returned to the sender of the message.
    pub fn emailbx(&self) -> &N {
        &self.emailbx
    }
}


//--- PartialEq and Eq

impl<N, NN> PartialEq<Minfo<NN>> for Minfo<N>
where N: ToDname, NN: ToDname {
    fn eq(&self, other: &Minfo<NN>) -> bool {
        self.rmailbx.name_eq(&other.rmailbx)
        && self.emailbx.name_eq(&other.emailbx)
    }
}

impl<N: ToDname> Eq for Minfo<N> { }


//--- PartialOrd, Ord, and CanonicalOrd

impl<N, NN> PartialOrd<Minfo<NN>> for Minfo<N>
where N: ToDname, NN: ToDname {
    fn partial_cmp(&self, other: &Minfo<NN>) -> Option<Ordering> {
        match self.rmailbx.name_cmp(&other.rmailbx) {
            Ordering::Equal => { }
            other => return Some(other)
        }
        Some(self.emailbx.name_cmp(&other.emailbx))
    }
}

impl<N: ToDname> Ord for Minfo<N> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.rmailbx.name_cmp(&other.rmailbx) {
            Ordering::Equal => { }
            other => return other
        }
        self.emailbx.name_cmp(&other.emailbx)
    }
}

impl<N: ToDname, NN: ToDname> CanonicalOrd<Minfo<NN>> for Minfo<N> {
    fn canonical_cmp(&self, other: &Minfo<NN>) -> Ordering {
        match self.rmailbx.lowercase_composed_cmp(&other.rmailbx) {
            Ordering::Equal => { }
            other => return other
        }
        self.emailbx.lowercase_composed_cmp(&other.emailbx)
    }
}


//--- Parse, ParseAll and Compose

impl<Octets, N: Parse<Octets>> Parse<Octets> for Minfo<N> {
    type Err = N::Err;

    fn parse(parser: &mut Parser<Octets>) -> Result<Self, Self::Err> {
        Ok(Self::new(N::parse(parser)?, N::parse(parser)?))
    }

    fn skip(parser: &mut Parser<Octets>) -> Result<(), Self::Err> {
        N::skip(parser)?;
        N::skip(parser)?;
        Ok(())
    }
}

impl<Octets, N> ParseAll<Octets> for Minfo<N>
where
    Octets: AsRef<[u8]>,
    N: Parse<Octets> + ParseAll<Octets>,
    <N as ParseAll<Octets>>::Err:
        From<<N as Parse<Octets>>::Err> + From<ShortBuf>
{
    type Err = <N as ParseAll<Octets>>::Err;

    fn parse_all(
        parser: &mut Parser<Octets>,
        len: usize
    ) -> Result<Self, Self::Err> {
        let pos = parser.pos();
        let rmailbx = N::parse(parser)?;
        let rlen = parser.pos() - pos;
        let len = if len <= rlen {
            // Because a domain name can never be empty, we seek back to the
            // beginning and reset the length to zero.
            parser.seek(pos)?;
            0
        }
        else {
            len - rlen
        };
        let emailbx = N::parse_all(parser, len)?;
        Ok(Self::new(rmailbx, emailbx))
    }
}

impl<N: ToDname> Compose for Minfo<N> {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_all(|target| {
            target.append_compressed_dname(&self.rmailbx)?;
            target.append_compressed_dname(&self.emailbx)
        })
    }

    fn compose_canonical<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_all(|target| {
            self.rmailbx.compose_canonical(target)?;
            self.emailbx.compose_canonical(target)
        })
    }
}


//--- Scan and Display

#[cfg(feature="bytes")] 
impl<N: Scan> Scan for Minfo<N> {
    fn scan<C: CharSource>(scanner: &mut  Scanner<C>)
                           -> Result<Self, ScanError> {
        Ok(Self::new(N::scan(scanner)?, N::scan(scanner)?))
    }
}

impl<N: fmt::Display> fmt::Display for Minfo<N> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}. {}.", self.rmailbx, self.emailbx)
    }
}


//--- RecordData

impl<N> RtypeRecordData for Minfo<N> {
    const RTYPE: Rtype = Rtype::Minfo;
}


//------------ Mr -----------------------------------------------------------

dname_type! {
    /// MR record data.
    ///
    /// The MR record specifices a mailbox which is the proper rename of the
    /// specified mailbox.
    /// 
    /// The MR record is experimental.
    ///
    /// The MR record type is defined in RFC 1035, section 3.3.8.
    (Mr, Mr, newname)
}


//------------ Mx -----------------------------------------------------------

/// Mx record data.
///
/// The Mx record specifies a host willing to serve as a mail exchange for
/// the owner name.
///
/// The Mx record type is defined in RFC 1035, section 3.3.9.
#[derive(Clone, Debug, Hash)]
pub struct Mx<N> {
    preference: u16,
    exchange: N,
}

impl<N> Mx<N> {
    /// Creates a new Mx record data from the components.
    pub fn new(preference: u16, exchange: N) -> Self {
        Mx { preference, exchange }
    }

    /// The preference for this record.
    ///
    /// Defines an order if there are several Mx records for the same owner.
    /// Lower values are preferred.
    pub fn preference(&self) -> u16 {
        self.preference
    }

    /// The name of the host that is the exchange.
    pub fn exchange(&self) -> &N {
        &self.exchange
    }
}


//--- PartialEq and Eq

impl<N, NN> PartialEq<Mx<NN>> for Mx<N>
where N: ToDname, NN: ToDname {
    fn eq(&self, other: &Mx<NN>) -> bool {
        self.preference == other.preference
        && self.exchange.name_eq(&other.exchange)
    }
}

impl<N: ToDname> Eq for Mx<N> { }


//--- PartialOrd, Ord, and CanonicalOrd

impl<N, NN> PartialOrd<Mx<NN>> for Mx<N>
where N: ToDname, NN: ToDname {
    fn partial_cmp(&self, other: &Mx<NN>) -> Option<Ordering> {
        match self.preference.partial_cmp(&other.preference) {
            Some(Ordering::Equal) => { }
            other => return other
        }
        Some(self.exchange.name_cmp(&other.exchange))
    }
}

impl<N: ToDname> Ord for Mx<N> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.preference.cmp(&other.preference) {
            Ordering::Equal => { }
            other => return other
        }
        self.exchange.name_cmp(&other.exchange)
    }
}

impl<N: ToDname, NN: ToDname> CanonicalOrd<Mx<NN>> for Mx<N> {
    fn canonical_cmp(&self, other: &Mx<NN>) -> Ordering {
        match self.preference.cmp(&other.preference) {
            Ordering::Equal => { }
            other => return other
        }
        self.exchange.lowercase_composed_cmp(&other.exchange)
    }
}


//--- Parse, ParseAll, Compose, Compress

impl<Octets: AsRef<[u8]>, N: Parse<Octets>> Parse<Octets> for Mx<N>
     where N::Err: From<ShortBuf> {
    type Err = N::Err;

    fn parse(parser: &mut Parser<Octets>) -> Result<Self, Self::Err> {
        Ok(Self::new(u16::parse(parser)?, N::parse(parser)?))
    }

    fn skip(parser: &mut Parser<Octets>) -> Result<(), Self::Err> {
        u16::skip(parser)?;
        N::skip(parser)
    }
}

impl<Octets: AsRef<[u8]>, N: ParseAll<Octets>> ParseAll<Octets> for Mx<N>
where N::Err: From<ParseOpenError> + From<ShortBuf> {
    type Err = N::Err;

    fn parse_all(
        parser: &mut Parser<Octets>,
        len: usize
    ) -> Result<Self, Self::Err> {
        if len < 3 {
            return Err(ParseOpenError::ShortField.into())
        }
        Ok(Self::new(u16::parse(parser)?, N::parse_all(parser, len - 2)?))
    }
}

impl<N: ToDname> Compose for Mx<N> {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_all(|target| {
            self.preference.compose(target)?;
            target.append_compressed_dname(&self.exchange)
        })
    }

    fn compose_canonical<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_all(|target| {
            self.preference.compose(target)?;
            self.exchange.compose_canonical(target)
        })
    }
}


//--- Scan and Display

#[cfg(feature="bytes")] 
impl<N: Scan> Scan for Mx<N> {
    fn scan<C: CharSource>(scanner: &mut Scanner<C>)
                           -> Result<Self, ScanError> {
        Ok(Self::new(u16::scan(scanner)?, N::scan(scanner)?))
    }
}

impl<N: fmt::Display> fmt::Display for Mx<N> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}.", self.preference, self.exchange)
    }
}


//--- RtypeRecordData

impl<N> RtypeRecordData for Mx<N> {
    const RTYPE: Rtype = Rtype::Mx;
}


//------------ Ns -----------------------------------------------------------

dname_type! {
    /// NS record data.
    ///
    /// NS records specify hosts that are authoritative for a class and domain.
    ///
    /// The NS record type is defined in RFC 1035, section 3.3.11.
    (Ns, Ns, nsdname)
}


//------------ Null ---------------------------------------------------------

/// Null record data.
///
/// Null records can contain whatever data. They are experimental and not
/// allowed in master files.
///
/// The Null record type is defined in RFC 1035, section 3.3.10.
#[derive(Clone)]
pub struct Null<Octets> {
    data: Octets,
}

impl<Octets> Null<Octets> {
    /// Creates new, empty owned Null record data.
    pub fn new(data: Octets) -> Self {
        Null { data }
    }

    /// The raw content of the record.
    pub fn data(&self) -> &Octets {
        &self.data
    }
}

impl<Octets: AsRef<[u8]>> Null<Octets> {
    pub fn len(&self) -> usize {
        self.data.as_ref().len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.as_ref().is_empty()
    }
}


//--- From

impl<Octets> From<Octets> for Null<Octets> {
    fn from(data: Octets) -> Self {
        Self::new(data)
    }
}


//--- PartialEq and Eq

impl<Octets, Other> PartialEq<Null<Other>> for Null<Octets>
where Octets: AsRef<[u8]>, Other: AsRef<[u8]> {
    fn eq(&self, other: &Null<Other>) -> bool {
        self.data.as_ref().eq(other.data.as_ref())
    }
}

impl<Octets: AsRef<[u8]>> Eq for Null<Octets> { }


//--- PartialOrd, CanonicalOrd, and Ord

impl<Octets, Other> PartialOrd<Null<Other>> for Null<Octets>
where Octets: AsRef<[u8]>, Other: AsRef<[u8]> {
    fn partial_cmp(&self, other: &Null<Other>) -> Option<Ordering> {
        self.data.as_ref().partial_cmp(other.data.as_ref())
    }
}

impl<Octets, Other> CanonicalOrd<Null<Other>> for Null<Octets>
where Octets: AsRef<[u8]>, Other: AsRef<[u8]> {
    fn canonical_cmp(&self, other: &Null<Other>) -> Ordering {
        self.data.as_ref().cmp(other.data.as_ref())
    }
}

impl<Octets: AsRef<[u8]>> Ord for Null<Octets> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.data.as_ref().cmp(other.data.as_ref())
    }
}


//--- Hash

impl<Octets: AsRef<[u8]>> hash::Hash for Null<Octets> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.data.as_ref().hash(state)
    }
}


//--- ParseAll and Compose

impl<Octets: ParseOctets> ParseAll<Octets> for Null<Octets> {
    type Err = ShortBuf;

    fn parse_all(
        parser: &mut Parser<Octets>,
        len: usize
    ) -> Result<Self, Self::Err> {
        parser.parse_octets(len).map(Self::new)
    }
}

impl<Octets: AsRef<[u8]>> Compose for Null<Octets> {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_slice(self.data.as_ref())
    }
}


//--- RtypeRecordData

impl<Octets> RtypeRecordData for Null<Octets> {
    const RTYPE: Rtype = Rtype::Null;
}


//--- Deref

impl<Octets> ops::Deref for Null<Octets> {
    type Target = Octets;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}


//--- AsRef

impl<Octets: AsRef<Other>, Other> AsRef<Other> for Null<Octets> {
    fn as_ref(&self) -> &Other {
        self.data.as_ref()
    }
}


//--- Display and Debug

impl<Octets: AsRef<[u8]>> fmt::Display for Null<Octets> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\\# {}", self.data.as_ref().len())?;
        for ch in self.data.as_ref().iter() {
            write!(f, " {:02x}", ch)?;
        }
        Ok(())
    }
}

impl<Octets: AsRef<[u8]>> fmt::Debug for Null<Octets> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Null(")?;
        fmt::Display::fmt(self, f)?;
        f.write_str(")")
    }
}


//------------ Ptr ----------------------------------------------------------

dname_type! {
    /// PTR record data.
    ///
    /// PRT records are used in special domains to point to some other location
    /// in the domain space.
    ///
    /// The PTR record type is defined in RFC 1035, section 3.3.12.
    (Ptr, Ptr, ptrdname)
}

impl<N> Ptr<N> {
    pub fn into_ptrdname(self) -> N {
        self.ptrdname
    }
}


//------------ Soa ----------------------------------------------------------

/// Soa record data.
///
/// Soa records mark the top of a zone and contain information pertinent to
/// name server maintenance operations.
///
/// The Soa record type is defined in RFC 1035, section 3.3.13.
#[derive(Clone, Debug, Hash)]
pub struct Soa<N> {
    mname: N,
    rname: N,
    serial: Serial,
    refresh: u32,
    retry: u32,
    expire: u32,
    minimum:u32 
}

impl<N> Soa<N> {
    /// Creates new Soa record data from content.
    pub fn new(mname: N, rname: N, serial: Serial,
               refresh: u32, retry: u32, expire: u32, minimum: u32) -> Self {
        Soa { mname, rname, serial, refresh, retry, expire, minimum }
    }

    /// The primary name server for the zone.
    pub fn mname(&self) -> &N {
        &self.mname
    }

    /// The mailbox for the person responsible for this zone.
    pub fn rname(&self) -> &N {
        &self.rname
    }

    /// The serial number of the original copy of the zone.
    pub fn serial(&self) -> Serial {
        self.serial
    }

    /// The time interval in seconds before the zone should be refreshed.
    pub fn refresh(&self) -> u32 {
        self.refresh
    }

    /// The time in seconds before a failed refresh is retried.
    pub fn retry(&self) -> u32 {
        self.retry
    }

    /// The upper limit of time in seconds the zone is authoritative.
    pub fn expire(&self) -> u32 {
        self.expire
    }

    /// The minimum TTL to be exported with any RR from this zone.
    pub fn minimum(&self) -> u32 {
        self.minimum
    }
}


//--- PartialEq and Eq

impl<N, NN> PartialEq<Soa<NN>> for Soa<N>
where N: ToDname, NN: ToDname {
    fn eq(&self, other: &Soa<NN>) -> bool {
        self.mname.name_eq(&other.mname)
        && self.rname.name_eq(&other.rname)
        && self.serial == other.serial && self.refresh == other.refresh
        && self.retry == other.retry && self.expire == other.expire
        && self.minimum == other.minimum
    }
}

impl<N: ToDname> Eq for Soa<N> { }


//--- PartialOrd, Ord, and CanonicalOrd

impl<N, NN> PartialOrd<Soa<NN>> for Soa<N>
where N: ToDname, NN: ToDname {
    fn partial_cmp(&self, other: &Soa<NN>) -> Option<Ordering> {
        match self.mname.name_cmp(&other.mname) {
            Ordering::Equal => { }
            other => return Some(other)
        }
        match self.rname.name_cmp(&other.rname) {
            Ordering::Equal => { }
            other => return Some(other)
        }
        match u32::from(self.serial).partial_cmp(&u32::from(other.serial)) {
            Some(Ordering::Equal) => { }
            other => return other
        }
        match self.refresh.partial_cmp(&other.refresh) {
            Some(Ordering::Equal) => { }
            other => return other
        }
        match self.retry.partial_cmp(&other.retry) {
            Some(Ordering::Equal) => { }
            other => return other
        }
        match self.expire.partial_cmp(&other.expire) {
            Some(Ordering::Equal) => { }
            other => return other
        }
        self.minimum.partial_cmp(&other.minimum)
    }
}

impl<N: ToDname> Ord for Soa<N> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.mname.name_cmp(&other.mname) {
            Ordering::Equal => { }
            other => return other
        }
        match self.rname.name_cmp(&other.rname) {
            Ordering::Equal => { }
            other => return other
        }
        match u32::from(self.serial).cmp(&u32::from(other.serial)) {
            Ordering::Equal => { }
            other => return other
        }
        match self.refresh.cmp(&other.refresh) {
            Ordering::Equal => { }
            other => return other
        }
        match self.retry.cmp(&other.retry) {
            Ordering::Equal => { }
            other => return other
        }
        match self.expire.cmp(&other.expire) {
            Ordering::Equal => { }
            other => return other
        }
        self.minimum.cmp(&other.minimum)
    }
}

impl<N: ToDname, NN: ToDname> CanonicalOrd<Soa<NN>> for Soa<N> {
    fn canonical_cmp(&self, other: &Soa<NN>) -> Ordering {
        match self.mname.lowercase_composed_cmp(&other.mname) {
            Ordering::Equal => { }
            other => return other
        }
        match self.rname.lowercase_composed_cmp(&other.rname) {
            Ordering::Equal => { }
            other => return other
        }
        match self.serial.canonical_cmp(&other.serial) {
            Ordering::Equal => { }
            other => return other
        }
        match self.refresh.cmp(&other.refresh) {
            Ordering::Equal => { }
            other => return other
        }
        match self.retry.cmp(&other.retry) {
            Ordering::Equal => { }
            other => return other
        }
        match self.expire.cmp(&other.expire) {
            Ordering::Equal => { }
            other => return other
        }
        self.minimum.cmp(&other.minimum)
    }
}


//--- Parse, ParseAll, and Compose

impl<Octets, N> Parse<Octets> for Soa<N>
where
    Octets: AsRef<[u8]>, N: Parse<Octets>,
    N::Err: From<ShortBuf>
{
    type Err = N::Err;

    fn parse(parser: &mut Parser<Octets>) -> Result<Self, Self::Err> {
        Ok(Self::new(
            N::parse(parser)?,
            N::parse(parser)?,
            Serial::parse(parser)?,
            u32::parse(parser)?,
            u32::parse(parser)?,
            u32::parse(parser)?,
            u32::parse(parser)?
        ))
    }

    fn skip(parser: &mut Parser<Octets>) -> Result<(), Self::Err> {
        N::skip(parser)?;
        N::skip(parser)?;
        Serial::skip(parser)?;
        u32::skip(parser)?;
        u32::skip(parser)?;
        u32::skip(parser)?;
        u32::skip(parser)?;
        Ok(())
    }
}

impl<Octets, N> ParseAll<Octets> for Soa<N>
where
    Octets: AsRef<[u8]> + Clone,
    N: ParseAll<Octets> + Parse<Octets>,
    <N as ParseAll<Octets>>::Err: From<<N as Parse<Octets>>::Err>,
    <N as ParseAll<Octets>>::Err: From<ParseAllError>,
    <N as Parse<Octets>>::Err: From<ShortBuf>
{
    type Err = <N as ParseAll<Octets>>::Err;

    fn parse_all(
        parser: &mut Parser<Octets>,
        len: usize
    ) -> Result<Self, Self::Err> {
        let mut tmp = parser.clone();
        let res = <Self as Parse<Octets>>::parse(&mut tmp)?;
        if tmp.pos() - parser.pos() < len {
            Err(ParseAllError::TrailingData.into())
        }
        else if tmp.pos() - parser.pos() > len {
            Err(ParseAllError::ShortField.into())
        }
        else {
            parser.advance(len)?;
            Ok(res)
        }
    }
}

impl<N: ToDname> Compose for Soa<N> {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_all(|buf| {
            buf.append_compressed_dname(&self.mname)?;
            buf.append_compressed_dname(&self.rname)?;
            self.serial.compose(buf)?;
            self.refresh.compose(buf)?;
            self.retry.compose(buf)?;
            self.expire.compose(buf)?;
            self.minimum.compose(buf)
        })
    }

    fn compose_canonical<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_all(|buf| {
            self.mname.compose_canonical(buf)?;
            self.rname.compose_canonical(buf)?;
            self.serial.compose(buf)?;
            self.refresh.compose(buf)?;
            self.retry.compose(buf)?;
            self.expire.compose(buf)?;
            self.minimum.compose(buf)
        })
    }
}


//--- Scan and Display

#[cfg(feature="bytes")] 
impl<N: Scan> Scan for Soa<N> {
    fn scan<C: CharSource>(scanner: &mut Scanner<C>)
                           -> Result<Self, ScanError> {
        Ok(Self::new(N::scan(scanner)?, N::scan(scanner)?,
                     Serial::scan(scanner)?, u32::scan(scanner)?,
                     u32::scan(scanner)?, u32::scan(scanner)?,
                     u32::scan(scanner)?))
    }
}

impl<N: fmt::Display> fmt::Display for Soa<N> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}. {}. {} {} {} {} {}",
               self.mname, self.rname, self.serial, self.refresh, self.retry,
               self.expire, self.minimum)
    }
}


//--- RecordData

impl<N> RtypeRecordData for Soa<N> {
    const RTYPE: Rtype = Rtype::Soa;
}


//------------ Txt ----------------------------------------------------------

/// Txt record data.
///
/// Txt records hold descriptive text.
///
/// The Txt record type is defined in RFC 1035, section 3.3.14.
#[derive(Clone)]
pub struct Txt<Octets>(Octets);

impl<Octets: FromBuilder> Txt<Octets> {
    /// Creates a new Txt record from a single character string.
    pub fn from_slice(text: &[u8]) -> Result<Self, PushError>
    where <Octets as FromBuilder>::Builder: EmptyBuilder {
        let mut builder = TxtBuilder::<Octets::Builder>::new();
        builder.append_slice(text)?;
        Ok(builder.finish())
    }
}

impl<Octets: AsRef<[u8]>> Txt<Octets> {
    /// Returns an iterator over the text items.
    ///
    /// The Txt format contains one or more length-delimited byte strings.
    /// This method returns an iterator over each of them.
    pub fn iter(&self) -> TxtIter {
        TxtIter(Parser::from_octets(self.0.as_ref()))
    }

    pub fn as_flat_slice(&self) -> Option<&[u8]> {
        if self.0.as_ref()[0] as usize == self.0.as_ref().len() - 1 {
            Some(&self.0.as_ref()[1..])
        }
        else {
            None
        }
    }
    
    pub fn len(&self) -> usize {
        self.0.as_ref().len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// Returns the text content.
    ///
    /// If the data is only one single character string, returns a simple
    /// clone of the slice with the data. If there are several character
    /// strings, their content will be copied together into one single,
    /// newly allocated bytes value.
    ///
    /// Access to the individual character strings is possible via iteration.
    pub fn text<T: FromBuilder>(&self) -> Result<T, ShortBuf>
    where <T as FromBuilder>::Builder: EmptyBuilder {
        // Capacity will be a few bytes too much. Probably better than
        // re-allocating.
        let mut res = T::Builder::with_capacity(self.len());
        for item in self.iter() {
            res.append_slice(item)?;
        }
        Ok(res.into_octets())
    }
}


//--- IntoIterator

impl<'a, Octets: AsRef<[u8]>> IntoIterator for &'a Txt<Octets> {
    type Item = &'a [u8];
    type IntoIter = TxtIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

//--- PartialEq and Eq

impl<Octets, Other> PartialEq<Txt<Other>> for Txt<Octets>
where Octets: AsRef<[u8]>, Other: AsRef<[u8]> {
    fn eq(&self, other: &Txt<Other>) -> bool {
        self.iter().flat_map(|s| s.iter().copied()).eq(
            other.iter().flat_map(|s| s.iter().copied())
        )
    }
}

impl<Octets: AsRef<[u8]>> Eq for Txt<Octets> { }


//--- PartialOrd, CanonicalOrd, and Ord

impl<Octets, Other> PartialOrd<Txt<Other>> for Txt<Octets>
where Octets: AsRef<[u8]>, Other: AsRef<[u8]> {
    fn partial_cmp(&self, other: &Txt<Other>) -> Option<Ordering> {
        self.iter().flat_map(|s| s.iter().copied()).partial_cmp(
            other.iter().flat_map(|s| s.iter().copied())
        )
    }
}

impl<Octets, Other> CanonicalOrd<Txt<Other>> for Txt<Octets>
where Octets: AsRef<[u8]>, Other: AsRef<[u8]> {
    fn canonical_cmp(&self, other: &Txt<Other>) -> Ordering {
        self.iter().flat_map(|s| s.iter().copied()).cmp(
            other.iter().flat_map(|s| s.iter().copied())
        )
    }
}

impl<Octets: AsRef<[u8]>> Ord for Txt<Octets> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().flat_map(|s| s.iter().copied()).cmp(
            other.iter().flat_map(|s| s.iter().copied())
        )
    }
}


//--- Hash

impl<Octets: AsRef<[u8]>> hash::Hash for Txt<Octets> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.iter().flat_map(|s| s.iter().copied())
            .for_each(|c| c.hash(state))
    }
}


//--- ParseAll and Compose

impl<Octets: ParseOctets> ParseAll<Octets> for Txt<Octets> {
    type Err = ParseOpenError;

    fn parse_all(
        parser: &mut Parser<Octets>,
        len: usize
    ) -> Result<Self, Self::Err> {
        let text = parser.parse_octets(len)?;
        let mut tmp = Parser::from_octets(text.clone());
        while parser.remaining() != 0 {
            CharStr::skip(&mut tmp)?
        }
        Ok(Txt(tmp.into_octets()))
    }
}

impl<Octets: AsRef<[u8]>> Compose for Txt<Octets> {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_slice(self.0.as_ref())
    }
}


//--- Scan and Display

#[cfg(feature="bytes")] 
impl Scan for Txt<Bytes> {
    fn scan<C: CharSource>(
        scanner: &mut Scanner<C>
    ) -> Result<Self, ScanError> {
        scanner.scan_byte_phrase(|res| {
            let mut builder = TxtBuilder::new_bytes();
            if builder.append_slice(res.as_ref()).is_err() {
                Err(SyntaxError::LongCharStr)
            }
            else {
                Ok(builder.finish())
            }
        })
    }
}

impl<Octets: AsRef<[u8]>> fmt::Display for Txt<Octets> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for slice in self.iter() {
            for ch in slice.iter() {
                fmt::Display::fmt(&Symbol::from_byte(*ch), f)?
            }
        }
        Ok(())
    }
}


//--- Debug

impl<Octets: AsRef<[u8]>> fmt::Debug for Txt<Octets> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Txt(")?;
        fmt::Display::fmt(self, f)?;
        f.write_str(")")
    }
}


//--- RtypeRecordData

impl<Octets> RtypeRecordData for Txt<Octets> {
    const RTYPE: Rtype = Rtype::Txt;
}


//------------ TxtIter -------------------------------------------------------

/// An iterator over the character strings of a Txt record.
#[derive(Clone)]
pub struct TxtIter<'a>(Parser<&'a [u8]>);

impl<'a> Iterator for TxtIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.remaining() == 0 {
            None
        }
        else {
            Some(unwrap!(CharStr::parse(&mut self.0)).into_octets())
        }
    }
}


//------------ TxtBuilder ---------------------------------------------------

#[derive(Clone, Debug)]
pub struct TxtBuilder<Builder> {
    builder: Builder,

    /// The index of the start of the current char string.
    ///
    /// If this is `None`, there currently is no char string being worked on.
    start: Option<usize>,
}

impl<Builder: OctetsBuilder + EmptyBuilder> TxtBuilder<Builder> {
    pub fn new() -> Self {
        TxtBuilder {
            builder: Builder::empty(),
            start: None
        }
    }
}

#[cfg(feature="bytes")] 
impl TxtBuilder<BytesMut> {
    pub fn new_bytes() -> Self {
        Self::new()
    }
}

impl<Builder: OctetsBuilder> TxtBuilder<Builder> {
    pub fn append_slice(&mut self, mut slice: &[u8]) -> Result<(), PushError> {
        if let Some(start) = self.start {
            let left = 255 - (self.builder.len() - (start + 1));
            if slice.len() < left {
                self.builder.append_slice(slice)?;
                return Ok(())
            }
            let (append, left) = slice.split_at(left);
            self.builder.append_slice(append)?;
            self.builder.as_mut()[start] = 255;
            slice = left;
        }
        for chunk in slice.chunks(255) {
            if self.builder.len() >= 0xFFFF {
                return Err(PushError);
            }
            self.start = if chunk.len() == 255 {
                None
            }
            else {
                Some(self.builder.len())
            };
            self.builder.append_slice(&[chunk.len() as u8])?;
            self.builder.append_slice(chunk)?;
        }
        Ok(())
    }

    pub fn finish(mut self) -> Txt<Builder::Octets>
    where Builder: IntoOctets {
        if let Some(start) = self.start {
            self.builder.as_mut()[start] = 
                (255 - (self.builder.len() - (start + 1))) as u8;
        }
        Txt(self.builder.into_octets())
    }
}


impl<Builder: OctetsBuilder + EmptyBuilder> Default for TxtBuilder<Builder> {
    fn default() -> Self {
        Self::new()
    }
}


//------------ Wks ----------------------------------------------------------

/// Wks record data.
///
/// Wks records describe the well-known services supported by a particular
/// protocol on a particular internet address.
///
/// The Wks record type is defined in RFC 1035, section 3.4.2.
#[derive(Clone)]
pub struct Wks<Octets> {
    address: Ipv4Addr,
    protocol: u8,
    bitmap: Octets,
}

impl<Octets> Wks<Octets> {
    /// Creates a new record data from components.
    pub fn new(address: Ipv4Addr, protocol: u8, bitmap: Octets) -> Self {
        Wks { address, protocol, bitmap }
    }

    /// The IPv4 address of the host this record refers to.
    pub fn address(&self) -> Ipv4Addr {
        self.address
    }

    /// The protocol number of the protocol this record refers to.
    ///
    /// This will typically be `6` for TCP or `17` for UDP.
    pub fn protocol(&self) -> u8 {
        self.protocol
    }

    /// A bitmap indicating the ports where service is being provided.
    pub fn bitmap(&self) -> &Octets {
        &self.bitmap
    }
}

impl<Octets: AsRef<[u8]>> Wks<Octets> {
    /// Returns whether a certain service is being provided.
    pub fn serves(&self, port: u16) -> bool {
        let octet = (port / 8) as usize;
        let bit = (port % 8) as usize;
        match self.bitmap.as_ref().get(octet) {
            Some(x) => (x >> bit) > 0,
            None => false
        }
    }

    /// Returns an iterator over the served ports.
    pub fn iter(&self) -> WksIter {
        WksIter::new(self.bitmap.as_ref())
    }
}


//--- PartialEq and Eq

impl<Octets, Other> PartialEq<Wks<Other>> for Wks<Octets>
where Octets: AsRef<[u8]>, Other: AsRef<[u8]> {
    fn eq(&self, other: &Wks<Other>) -> bool {
        self.address == other.address
        && self.protocol == other.protocol
        && self.bitmap.as_ref() == other.bitmap.as_ref()
    }
}

impl<Octets: AsRef<[u8]>> Eq for Wks<Octets> { }


//--- PartialOrd, CanonicalOrd, and Ord

impl<Octets, Other> PartialOrd<Wks<Other>> for Wks<Octets>
where Octets: AsRef<[u8]>, Other: AsRef<[u8]> {
    fn partial_cmp(&self, other: &Wks<Other>) -> Option<Ordering> {
        match self.address.octets().partial_cmp(&other.address.octets()) {
            Some(Ordering::Equal) => { }
            other => return other
        }
        match self.protocol.partial_cmp(&other.protocol) {
            Some(Ordering::Equal) => { }
            other => return other
        }
        self.bitmap.as_ref().partial_cmp(other.bitmap.as_ref())
    }
}

impl<Octets, Other> CanonicalOrd<Wks<Other>> for Wks<Octets>
where Octets: AsRef<[u8]>, Other: AsRef<[u8]> {
    fn canonical_cmp(&self, other: &Wks<Other>) -> Ordering {
        match self.address.octets().cmp(&other.address.octets()) {
            Ordering::Equal => { }
            other => return other
        }
        match self.protocol.cmp(&other.protocol) {
            Ordering::Equal => { }
            other => return other
        }
        self.bitmap.as_ref().cmp(other.bitmap.as_ref())
    }
}

impl<Octets: AsRef<[u8]>> Ord for Wks<Octets> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.address.octets().cmp(&other.address.octets()) {
            Ordering::Equal => { }
            other => return other
        }
        match self.protocol.cmp(&other.protocol) {
            Ordering::Equal => { }
            other => return other
        }
        self.bitmap.as_ref().cmp(other.bitmap.as_ref())
    }
}


//--- Hash

impl<Octets: AsRef<[u8]>> hash::Hash for Wks<Octets> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.address.hash(state);
        self.protocol.hash(state);
        self.bitmap.as_ref().hash(state);
    }
}


//--- ParseAll, Compose, Compress

impl<Octets: ParseOctets> ParseAll<Octets> for Wks<Octets> {
    type Err = ParseOpenError;

    fn parse_all(
        parser: &mut Parser<Octets>,
        len: usize
    ) -> Result<Self, Self::Err> {
        if len < 5 {
            return Err(ParseOpenError::ShortField)
        }
        Ok(Self::new(
            Ipv4Addr::parse(parser)?,
            u8::parse(parser)?,
            parser.parse_octets(len - 5)?
        ))
    }
}

impl<Octets: AsRef<[u8]>> Compose for Wks<Octets> {
    fn compose<T: OctetsBuilder>(
        &self,
        target: &mut T
    ) -> Result<(), ShortBuf> {
        target.append_all(|target| {
            self.address.compose(target)?;
            self.protocol.compose(target)?;
            target.append_slice(self.bitmap.as_ref())
        })
    }
}


//--- Scan and Display

#[cfg(feature="bytes")] 
impl Scan for Wks<Bytes> {
    fn scan<C: CharSource>(
        scanner: &mut Scanner<C>
    ) -> Result<Self, ScanError> {
        let address = scanner.scan_string_phrase(|res| {
            Ipv4Addr::from_str(&res).map_err(Into::into)
        })?;
        let protocol = u8::scan(scanner)?;
        let mut builder = WksBuilder::new_bytes(address, protocol);
        while let Ok(service) = u16::scan(scanner) {
            builder.add_service(service)
        }
        Ok(builder.finish())
    }
}

impl<Octets: AsRef<[u8]>> fmt::Display for Wks<Octets> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.address, self.protocol)?;
        for service in self.iter() {
            write!(f, " {}", service)?;
        }
        Ok(())
    }
}


//--- Debug

impl<Octets: AsRef<[u8]>> fmt::Debug for Wks<Octets> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Wks(")?;
        fmt::Display::fmt(self, f)?;
        f.write_str(")")
    }
}


//--- RecordData

impl<Octets> RtypeRecordData for Wks<Octets> {
    const RTYPE: Rtype = Rtype::Wks;
}


//------------ WksIter -------------------------------------------------------

/// An iterator over the services active in a Wks record.
///
/// This iterates over the port numbers in growing order.
#[derive(Clone, Debug)]
pub struct WksIter<'a> {
    bitmap: &'a [u8],
    octet: usize,
    bit: usize
}

impl<'a> WksIter<'a> {
    fn new(bitmap: &'a [u8]) -> Self {
        WksIter { bitmap, octet: 0, bit: 0 }
    }

    fn serves(&self) -> bool {
        (self.bitmap[self.octet] >> self.bit) > 0
    }
}

impl<'a> Iterator for WksIter<'a> {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.octet >= self.bitmap.len() { return None }
            else {
                if self.serves() {
                    return Some((self.octet * 8 + self.bit) as u16)
                }
                if self.bit == 7 { self.octet += 1; self.bit = 0 }
                else { self.bit += 1 }
            }
        }
    }
}


//------------ WksBuilder ----------------------------------------------------

#[derive(Clone, Debug)]
pub struct WksBuilder<Builder> {
    address: Ipv4Addr,
    protocol: u8,
    bitmap: Builder,
}

impl<Builder: OctetsBuilder + EmptyBuilder> WksBuilder<Builder> {
    pub fn new(address: Ipv4Addr, protocol: u8) -> Self {
        WksBuilder { address, protocol, bitmap: Builder::empty() }
    }
}

#[cfg(feature="bytes")] 
impl WksBuilder<BytesMut> {
    pub fn new_bytes(address: Ipv4Addr, protocol: u8) -> Self {
        Self::new(address, protocol)
    }
}

impl<Builder: OctetsBuilder> WksBuilder<Builder> {
    pub fn add_service(&mut self, service: u16) -> Result<(), ShortBuf> {
        let octet = (service >> 2) as usize;
        let bit = 1 << (service & 0x3);
        while self.bitmap.len() < octet + 1 {
            self.bitmap.append_slice(b"0")?
        }
        self.bitmap.as_mut()[octet] |= bit;
        Ok(())
    }

    pub fn finish(self) -> Wks<Builder::Octets>
    where Builder: IntoOctets + EmptyBuilder {
        Wks::new(self.address, self.protocol, self.bitmap.into_octets())
    }
}


//------------ parsed sub-module ---------------------------------------------

pub mod parsed {
    use crate::name::ParsedDname;

    pub use super::A;
    pub type Cname<Octets> = super::Cname<ParsedDname<Octets>>;
    pub use super::Hinfo;
    pub type Mb<Octets> = super::Mb<ParsedDname<Octets>>;
    pub type Md<Octets> = super::Md<ParsedDname<Octets>>;
    pub type Mf<Octets> = super::Mf<ParsedDname<Octets>>;
    pub type Mg<Octets> = super::Mg<ParsedDname<Octets>>;
    pub type Minfo<Octets> = super::Minfo<ParsedDname<Octets>>;
    pub type Mr<Octets> = super::Mr<ParsedDname<Octets>>;
    pub type Mx<Octets> = super::Mx<ParsedDname<Octets>>;
    pub type Ns<Octets> = super::Ns<ParsedDname<Octets>>;
    pub use super::Null;
    pub type Ptr<Octets> = super::Ptr<ParsedDname<Octets>>;
    pub type Soa<Octets> = super::Soa<ParsedDname<Octets>>;
    pub use super::Txt;
    pub use super::Wks;
}

