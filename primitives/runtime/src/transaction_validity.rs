// Copyright 2017-2020 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Transaction validity interface.

use sp_std::prelude::*;
use crate::codec::{Encode, Decode};
use crate::RuntimeDebug;

/// Priority for a transaction. Additive. Higher is better.
pub type TransactionPriority = u64;

/// Minimum number of blocks a transaction will remain valid for.
/// `TransactionLongevity::max_value()` means "forever".
pub type TransactionLongevity = u64;

/// Tag for a transaction. No two transactions with the same tag should be placed on-chain.
pub type TransactionTag = Vec<u8>;

/// An invalid transaction validity.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Copy, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(serde::Serialize))]
pub enum InvalidTransaction {
	/// The call of the transaction is not expected.
	Call,
	/// General error to do with the inability to pay some fees (e.g. account balance too low).
	Payment,
	/// General error to do with the transaction not yet being valid (e.g. nonce too high).
	Future,
	/// General error to do with the transaction being outdated (e.g. nonce too low).
	Stale,
	/// General error to do with the transaction's proofs (e.g. signature).
	BadProof,
	/// The transaction birth block is ancient.
	AncientBirthBlock,
	/// The transaction would exhaust the resources of current block.
	///
	/// The transaction might be valid, but there are not enough resources left in the current block.
	ExhaustsResources,
	/// Any other custom invalid validity that is not covered by this enum.
	Custom(u8),
	/// An extrinsic with a Mandatory dispatch resulted in Error. This is indicative of either a
	/// malicious validator or a buggy `provide_inherent`. In any case, it can result in dangerously
	/// overweight blocks and therefore if found, invalidates the block.
	BadMandatory,
	/// A transaction with a mandatory dispatch. This is invalid; only inherent extrinsics are
	/// allowed to have mandatory dispatches.
	MandatoryDispatch,
}

impl InvalidTransaction {
	/// Returns if the reason for the invalidity was block resource exhaustion.
	pub fn exhausted_resources(&self) -> bool {
		match self {
			Self::ExhaustsResources => true,
			_ => false,
		}
	}

	/// Returns if the reason for the invalidity was a mandatory call failing.
	pub fn was_mandatory(&self) -> bool {
		match self {
			Self::BadMandatory => true,
			_ => false,
		}
	}
}

impl From<InvalidTransaction> for &'static str {
	fn from(invalid: InvalidTransaction) -> &'static str {
		match invalid {
			InvalidTransaction::Call => "Transaction call is not expected",
			InvalidTransaction::Future => "Transaction will be valid in the future",
			InvalidTransaction::Stale => "Transaction is outdated",
			InvalidTransaction::BadProof => "Transaction has a bad signature",
			InvalidTransaction::AncientBirthBlock => "Transaction has an ancient birth block",
			InvalidTransaction::ExhaustsResources =>
				"Transaction would exhausts the block limits",
			InvalidTransaction::Payment =>
				"Inability to pay some fees (e.g. account balance too low)",
			InvalidTransaction::BadMandatory =>
				"A call was labelled as mandatory, but resulted in an Error.",
			InvalidTransaction::MandatoryDispatch =>
				"Tranaction dispatch is mandatory; transactions may not have mandatory dispatches.",
			InvalidTransaction::Custom(_) => "InvalidTransaction custom error",
		}
	}
}

/// An unknown transaction validity.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Copy, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(serde::Serialize))]
pub enum UnknownTransaction {
	/// Could not lookup some information that is required to validate the transaction.
	CannotLookup,
	/// No validator found for the given unsigned transaction.
	NoUnsignedValidator,
	/// Any other custom unknown validity that is not covered by this enum.
	Custom(u8),
}

impl From<UnknownTransaction> for &'static str {
	fn from(unknown: UnknownTransaction) -> &'static str {
		match unknown {
			UnknownTransaction::CannotLookup =>
				"Could not lookup information required to validate the transaction",
			UnknownTransaction::NoUnsignedValidator =>
				"Could not find an unsigned validator for the unsigned transaction",
			UnknownTransaction::Custom(_) => "UnknownTransaction custom error",
		}
	}
}

/// Errors that can occur while checking the validity of a transaction.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Copy, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(serde::Serialize))]
pub enum TransactionValidityError {
	/// The transaction is invalid.
	Invalid(InvalidTransaction),
	/// Transaction validity can't be determined.
	Unknown(UnknownTransaction),
}

impl TransactionValidityError {
	/// Returns `true` if the reason for the error was block resource exhaustion.
	pub fn exhausted_resources(&self) -> bool {
		match self {
			Self::Invalid(e) => e.exhausted_resources(),
			Self::Unknown(_) => false,
		}
	}

	/// Returns `true` if the reason for the error was it being a mandatory dispatch that could not
	/// be completed successfully.
	pub fn was_mandatory(&self) -> bool {
		match self {
			Self::Invalid(e) => e.was_mandatory(),
			Self::Unknown(_) => false,
		}
	}
}

impl From<TransactionValidityError> for &'static str {
	fn from(err: TransactionValidityError) -> &'static str {
		match err {
			TransactionValidityError::Invalid(invalid) => invalid.into(),
			TransactionValidityError::Unknown(unknown) => unknown.into(),
		}
	}
}

impl From<InvalidTransaction> for TransactionValidityError {
	fn from(err: InvalidTransaction) -> Self {
		TransactionValidityError::Invalid(err)
	}
}

impl From<UnknownTransaction> for TransactionValidityError {
	fn from(err: UnknownTransaction) -> Self {
		TransactionValidityError::Unknown(err)
	}
}

/// Information on a transaction's validity and, if valid, on how it relates to other transactions.
pub type TransactionValidity = Result<ValidTransaction, TransactionValidityError>;

impl Into<TransactionValidity> for InvalidTransaction {
	fn into(self) -> TransactionValidity {
		Err(self.into())
	}
}

impl Into<TransactionValidity> for UnknownTransaction {
	fn into(self) -> TransactionValidity {
		Err(self.into())
	}
}

/// The source of the transaction.
///
/// Depending on the source we might apply different validation schemes.
/// For instance we can disallow specific kinds of transactions if they were not produced
/// by our local node (for instance off-chain workers).
#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, parity_util_mem::MallocSizeOf)]
pub enum TransactionSource {
	/// Transaction is already included in block.
	///
	/// This means that we can't really tell where the transaction is coming from,
	/// since it's already in the received block. Note that the custom validation logic
	/// using either `Local` or `External` should most likely just allow `InBlock`
	/// transactions as well.
	InBlock,

	/// Transaction is coming from a local source.
	///
	/// This means that the transaction was produced internally by the node
	/// (for instance an Off-Chain Worker, or an Off-Chain Call), as opposed
	/// to being received over the network.
	Local,

	/// Transaction has been received externally.
	///
	/// This means the transaction has been received from (usually) "untrusted" source,
	/// for instance received over the network or RPC.
	External,
}

/// Information concerning a valid transaction.
#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
pub struct ValidTransaction {
	/// Priority of the transaction.
	///
	/// Priority determines the ordering of two transactions that have all
	/// their dependencies (required tags) satisfied.
	pub priority: TransactionPriority,
	/// Transaction dependencies
	///
	/// A non-empty list signifies that some other transactions which provide
	/// given tags are required to be included before that one.
	pub requires: Vec<TransactionTag>,
	/// Provided tags
	///
	/// A list of tags this transaction provides. Successfully importing the transaction
	/// will enable other transactions that depend on (require) those tags to be included as well.
	/// Provided and required tags allow Substrate to build a dependency graph of transactions
	/// and import them in the right (linear) order.
	pub provides: Vec<TransactionTag>,
	/// Transaction longevity
	///
	/// Longevity describes minimum number of blocks the validity is correct.
	/// After this period transaction should be removed from the pool or revalidated.
	pub longevity: TransactionLongevity,
	/// A flag indicating if the transaction should be propagated to other peers.
	///
	/// By setting `false` here the transaction will still be considered for
	/// including in blocks that are authored on the current node, but will
	/// never be sent to other peers.
	pub propagate: bool,
}

impl Default for ValidTransaction {
	fn default() -> Self {
		ValidTransaction {
			priority: 0,
			requires: vec![],
			provides: vec![],
			longevity: TransactionLongevity::max_value(),
			propagate: true,
		}
	}
}

impl ValidTransaction {
	/// Initiate `ValidTransaction` builder object with a particular prefix for tags.
	///
	/// To avoid conflicts between different pallets it's recommended to build `requires`
	/// and `provides` tags with a unique prefix.
	pub fn for_pallet(prefix: &'static str) -> ValidTransactionBuilder {
		ValidTransactionBuilder {
			prefix: Some(prefix),
			validity: Default::default(),
		}
	}

	/// Combine two instances into one, as a best effort. This will take the superset of each of the
	/// `provides` and `requires` tags, it will sum the priorities, take the minimum longevity and
	/// the logic *And* of the propagate flags.
	pub fn combine_with(mut self, mut other: ValidTransaction) -> Self {
		ValidTransaction {
			priority: self.priority.saturating_add(other.priority),
			requires: { self.requires.append(&mut other.requires); self.requires },
			provides: { self.provides.append(&mut other.provides); self.provides },
			longevity: self.longevity.min(other.longevity),
			propagate: self.propagate && other.propagate,
		}
	}
}

/// `ValidTransaction` builder.
///
///
/// Allows to easily construct `ValidTransaction` and most importantly takes care of
/// prefixing `requires` and `provides` tags to avoid pallet conflicts.
#[derive(Default, Clone, RuntimeDebug)]
pub struct ValidTransactionBuilder {
	prefix: Option<&'static str>,
	validity: ValidTransaction,
}

impl ValidTransactionBuilder {
	/// Set the priority of a transaction.
	///
	/// Note that the final priority for `FRAME` is combined from all `SignedExtension`s.
	/// Most likely for unsigned transactions you want the priority to be higher
	/// than for regular transactions. We recommend exposing a base priority for unsigned
	/// transactions as a pallet parameter, so that the runtime can tune inter-pallet priorities.
	pub fn priority(mut self, priority: TransactionPriority) -> Self {
		self.validity.priority = priority;
		self
	}

	/// Set the longevity of a transaction.
	///
	/// By default the transaction will be considered valid forever and will not be revalidated
	/// by the transaction pool. It's recommended though to set the longevity to a finite value
	/// though. If unsure, it's also reasonable to expose this parameter via pallet configuration
	/// and let the runtime decide.
	pub fn longevity(mut self, longevity: TransactionLongevity) -> Self {
		self.validity.longevity = longevity;
		self
	}

	/// Set the propagate flag.
	///
	/// Set to `false` if the transaction is not meant to be gossiped to peers. Combined with
	/// `TransactionSource::Local` validation it can be used to have special kind of
	/// transactions that are only produced and included by the validator nodes.
	pub fn propagate(mut self, propagate: bool) -> Self {
		self.validity.propagate = propagate;
		self
	}

	/// Add a `TransactionTag` to the set of required tags.
	///
	/// The tag will be encoded and prefixed with pallet prefix (if any).
	/// If you'd rather add a raw `require` tag, consider using `#combine_with` method.
	pub fn and_requires(mut self, tag: impl Encode) -> Self {
		self.validity.requires.push(match self.prefix.as_ref() {
			Some(prefix) => (prefix, tag).encode(),
			None => tag.encode(),
		});
		self
	}

	/// Add a `TransactionTag` to the set of provided tags.
	///
	/// The tag will be encoded and prefixed with pallet prefix (if any).
	/// If you'd rather add a raw `require` tag, consider using `#combine_with` method.
	pub fn and_provides(mut self, tag: impl Encode) -> Self {
		self.validity.provides.push(match self.prefix.as_ref() {
			Some(prefix) => (prefix, tag).encode(),
			None => tag.encode(),
		});
		self
	}

	/// Augment the builder with existing `ValidTransaction`.
	///
	/// This method does add the prefix to `require` or `provides` tags.
	pub fn combine_with(mut self, validity: ValidTransaction) -> Self {
		self.validity = core::mem::take(&mut self.validity).combine_with(validity);
		self
	}

	/// Finalize the builder and produce `TransactionValidity`.
	///
	/// Note the result will always be `Ok`. Use `Into` to produce `ValidTransaction`.
	pub fn build(self) -> TransactionValidity {
		self.into()
	}
}

impl From<ValidTransactionBuilder> for TransactionValidity {
	fn from(builder: ValidTransactionBuilder) -> Self {
		Ok(builder.into())
	}
}

impl From<ValidTransactionBuilder> for ValidTransaction {
	fn from(builder: ValidTransactionBuilder) -> Self {
		builder.validity
	}
}


#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn should_encode_and_decode() {
		let v: TransactionValidity = Ok(ValidTransaction {
			priority: 5,
			requires: vec![vec![1, 2, 3, 4]],
			provides: vec![vec![4, 5, 6]],
			longevity: 42,
			propagate: false,
		});

		let encoded = v.encode();
		assert_eq!(
			encoded,
			vec![0, 5, 0, 0, 0, 0, 0, 0, 0, 4, 16, 1, 2, 3, 4, 4, 12, 4, 5, 6, 42, 0, 0, 0, 0, 0, 0, 0, 0]
		);

		// decode back
		assert_eq!(TransactionValidity::decode(&mut &*encoded), Ok(v));
	}

	#[test]
	fn builder_should_prefix_the_tags() {
		const PREFIX: &str = "test";
		let a: ValidTransaction = ValidTransaction::for_pallet(PREFIX)
			.and_requires(1)
			.and_requires(2)
			.and_provides(3)
			.and_provides(4)
			.propagate(false)
			.longevity(5)
			.priority(3)
			.priority(6)
			.into();
		assert_eq!(a, ValidTransaction {
			propagate: false,
			longevity: 5,
			priority: 6,
			requires: vec![(PREFIX, 1).encode(), (PREFIX, 2).encode()],
			provides: vec![(PREFIX, 3).encode(), (PREFIX, 4).encode()],
		});
	}
}
