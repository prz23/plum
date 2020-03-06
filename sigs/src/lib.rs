// Copyright 2019-2020 PolkaX Authors. Licensed under GPL-3.0.

mod bls;
mod secp256k1;

pub use crate::bls::{bls_generate_secret, bls_sign};
pub use crate::secp256k1::{secp256k1_generate_secret, secp256k1_sign};