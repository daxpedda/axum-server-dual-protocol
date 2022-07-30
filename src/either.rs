//! This is a copy of
//! <https://github.com/hyperium/http-body/blob/6d7dd177dcbea2ffd1067f9f344beda7af5c78f2/http-body-util/src/either.rs>
//! and will be unnecessary if the crate is released and dependencies update or
//! `Either` is backported to `http-body` 0.4. See
//! <https://github.com/hyperium/http-body/issues/66>.

#![allow(unsafe_code, warnings)]

use std::error::Error;
use std::fmt::Debug;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Buf;
use http::HeaderMap;
use hyper::body::{HttpBody, SizeHint};
use proj::EitherProj;

/// Sum type with two cases: `Left` and `Right`, used if a body can be one of
/// two distinct types.
#[derive(Debug, Clone, Copy)]
pub enum Either<L, R> {
	/// A value of type `L`
	Left(L),
	/// A value of type `R`
	Right(R),
}

impl<L, R> Either<L, R> {
	/// This function is part of the generated code from `pin-project-lite`,
	/// for a more in depth explanation and the rest of the generated code refer
	/// to the [`proj`] module.
	pub(crate) fn project(self: Pin<&mut Self>) -> EitherProj<L, R> {
		unsafe {
			match self.get_unchecked_mut() {
				Self::Left(left) => EitherProj::Left(Pin::new_unchecked(left)),
				Self::Right(right) => EitherProj::Right(Pin::new_unchecked(right)),
			}
		}
	}
}

impl<L> Either<L, L> {
	/// Convert [`Either`] into the inner type, if both `Left` and `Right` are
	/// of the same type.
	pub fn into_inner(self) -> L {
		match self {
			Either::Left(left) => left,
			Either::Right(right) => right,
		}
	}
}

impl<L, R, Data> HttpBody for Either<L, R>
where
	L: HttpBody<Data = Data>,
	R: HttpBody<Data = Data>,
	L::Error: Into<Box<dyn Error + Send + Sync>>,
	R::Error: Into<Box<dyn Error + Send + Sync>>,
	Data: Buf,
{
	type Data = Data;
	type Error = Box<dyn Error + Send + Sync>;

	fn poll_data(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
	) -> Poll<Option<Result<Self::Data, Self::Error>>> {
		match self.project() {
			EitherProj::Left(left) => left
				.poll_data(cx)
				.map(|poll| poll.map(|opt| opt.map_err(Into::into))),
			EitherProj::Right(right) => right
				.poll_data(cx)
				.map(|poll| poll.map(|opt| opt.map_err(Into::into))),
		}
	}

	fn poll_trailers(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
	) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
		match self.project() {
			EitherProj::Left(left) => left.poll_trailers(cx).map(|poll| poll.map_err(Into::into)),
			EitherProj::Right(right) => {
				right.poll_trailers(cx).map(|poll| poll.map_err(Into::into))
			}
		}
	}

	fn is_end_stream(&self) -> bool {
		match self {
			Either::Left(left) => left.is_end_stream(),
			Either::Right(right) => right.is_end_stream(),
		}
	}

	fn size_hint(&self) -> SizeHint {
		match self {
			Either::Left(left) => left.size_hint(),
			Either::Right(right) => right.size_hint(),
		}
	}
}

pub(crate) mod proj {
	use std::marker::PhantomData;
	use std::pin::Pin;

	use super::Either;

	#[allow(dead_code)]
	#[allow(single_use_lifetimes)]
	#[allow(unknown_lints)]
	#[allow(clippy::mut_mut)]
	#[allow(clippy::redundant_pub_crate)]
	#[allow(clippy::ref_option_ref)]
	#[allow(clippy::type_repetition_in_bounds)]
	pub(crate) enum EitherProj<'__pin, L, R>
	where
		Either<L, R>: '__pin,
	{
		Left(Pin<&'__pin mut L>),
		Right(Pin<&'__pin mut R>),
	}

	#[allow(single_use_lifetimes)]
	#[allow(unknown_lints)]
	#[allow(clippy::used_underscore_binding)]
	#[allow(missing_debug_implementations)]
	const _: () = {
		#[allow(non_snake_case)]
		pub struct __Origin<'__pin, L, R> {
			__dummy_lifetime: PhantomData<&'__pin ()>,
			Left: L,
			Right: R,
		}
		impl<'__pin, L, R> Unpin for Either<L, R> where __Origin<'__pin, L, R>: Unpin {}

		trait MustNotImplDrop {}
		#[allow(drop_bounds)]
		impl<T: Drop> MustNotImplDrop for T {}
		impl<L, R> MustNotImplDrop for Either<L, R> {}
	};
}
