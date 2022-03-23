#![allow(unused_imports, unused_variables, dead_code, unused_assignments)]

use crate::xcon::{Formatter, Message, Parser, Request, XEvent, XconError};
use bstr::BStr;
use std::borrow::Cow;
use std::rc::Rc;
use uapi::OwnedFd;

include!(concat!(env!("OUT_DIR"), "/wire_xcon.rs"));