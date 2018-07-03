// Copyright (C) 2018 Stephane Raux. Distributed under the MIT license.

//! Extensions to the futures crate.

#![deny(missing_docs)]
#![deny(warnings)]

extern crate either;
#[macro_use]
extern crate futures;

pub mod sink;

pub use sink::SinkTools;
