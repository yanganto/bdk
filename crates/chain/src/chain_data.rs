use bitcoin::{hashes::Hash, BlockHash, OutPoint, TxOut, Txid};

use crate::{Anchor, AnchorFromBlockPosition, COINBASE_MATURITY};

/// Represents the observed position of some chain data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, core::hash::Hash)]
pub enum ChainPosition<A: Anchor> {
    /// The chain data is seen as confirmed, and in anchored by `A`.
    Confirmed(A),
    /// The chain data is seen in mempool at this given timestamp.
    Unconfirmed(u64),
}

impl<A: Anchor> ChainPosition<A> {
    /// Returns whether [`ChainPosition`] is confirmed or not.
    pub fn is_confirmed(&self) -> bool {
        matches!(self, Self::Confirmed(_))
    }
}

impl<A: Clone + Anchor> ChainPosition<&A> {
    /// Maps a [`ChainPosition<&A>`] into a [`ChainPosition<A>`] by cloning the contents.
    pub fn cloned(self) -> ChainPosition<A> {
        match self {
            ChainPosition::Confirmed(a) => ChainPosition::Confirmed(a.clone()),
            ChainPosition::Unconfirmed(last_seen) => ChainPosition::Unconfirmed(last_seen),
        }
    }
}

impl<A: Anchor> ChainPosition<A> {
    /// Determines the upper bound of the confirmation height.
    pub fn confirmation_height_upper_bound(&self) -> Option<u32> {
        match self {
            ChainPosition::Confirmed(a) => Some(a.confirmation_height_upper_bound()),
            ChainPosition::Unconfirmed(_) => None,
        }
    }
}

/// Block height and timestamp at which a transaction is confirmed.
#[derive(Debug, Clone, PartialEq, Eq, Copy, PartialOrd, Ord, core::hash::Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Deserialize, serde::Serialize),
    serde(crate = "serde_crate")
)]
pub enum ConfirmationTime {
    /// The confirmed variant.
    Confirmed {
        /// Confirmation height.
        height: u32,
        /// Confirmation time in unix seconds.
        time: u64,
    },
    /// The unconfirmed variant.
    Unconfirmed {
        /// The last-seen timestamp in unix seconds.
        last_seen: u64,
    },
}

impl ConfirmationTime {
    /// Construct an unconfirmed variant using the given `last_seen` time in unix seconds.
    pub fn unconfirmed(last_seen: u64) -> Self {
        Self::Unconfirmed { last_seen }
    }

    /// Returns whether [`ConfirmationTime`] is the confirmed variant.
    pub fn is_confirmed(&self) -> bool {
        matches!(self, Self::Confirmed { .. })
    }
}

impl From<ChainPosition<ConfirmationTimeHeightAnchor>> for ConfirmationTime {
    fn from(observed_as: ChainPosition<ConfirmationTimeHeightAnchor>) -> Self {
        match observed_as {
            ChainPosition::Confirmed(a) => Self::Confirmed {
                height: a.confirmation_height,
                time: a.confirmation_time,
            },
            ChainPosition::Unconfirmed(last_seen) => Self::Unconfirmed { last_seen },
        }
    }
}

/// A reference to a block in the canonical chain.
///
/// `BlockId` implements [`Anchor`]. When a transaction is anchored to `BlockId`, the confirmation
/// block and anchor block are the same block.
#[derive(Debug, Clone, PartialEq, Eq, Copy, PartialOrd, Ord, core::hash::Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Deserialize, serde::Serialize),
    serde(crate = "serde_crate")
)]
pub struct BlockId {
    /// The height of the block.
    pub height: u32,
    /// The hash of the block.
    pub hash: BlockHash,
}

impl Anchor for BlockId {
    fn anchor_block(&self) -> Self {
        *self
    }
}

impl AnchorFromBlockPosition for BlockId {
    fn from_block_position(_block: &bitcoin::Block, block_id: BlockId, _tx_pos: usize) -> Self {
        block_id
    }
}

impl Default for BlockId {
    fn default() -> Self {
        Self {
            height: Default::default(),
            hash: BlockHash::all_zeros(),
        }
    }
}

impl From<(u32, BlockHash)> for BlockId {
    fn from((height, hash): (u32, BlockHash)) -> Self {
        Self { height, hash }
    }
}

impl From<BlockId> for (u32, BlockHash) {
    fn from(block_id: BlockId) -> Self {
        (block_id.height, block_id.hash)
    }
}

impl From<(&u32, &BlockHash)> for BlockId {
    fn from((height, hash): (&u32, &BlockHash)) -> Self {
        Self {
            height: *height,
            hash: *hash,
        }
    }
}

/// An [`Anchor`] implementation that also records the exact confirmation height of the transaction.
///
/// Note that the confirmation block and the anchor block can be different here.
///
/// Refer to [`Anchor`] for more details.
#[derive(Debug, Default, Clone, PartialEq, Eq, Copy, PartialOrd, Ord, core::hash::Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Deserialize, serde::Serialize),
    serde(crate = "serde_crate")
)]
pub struct ConfirmationHeightAnchor {
    /// The anchor block.
    pub anchor_block: BlockId,

    /// The exact confirmation height of the transaction.
    ///
    /// It is assumed that this value is never larger than the height of the anchor block.
    pub confirmation_height: u32,
}

impl Anchor for ConfirmationHeightAnchor {
    fn anchor_block(&self) -> BlockId {
        self.anchor_block
    }

    fn confirmation_height_upper_bound(&self) -> u32 {
        self.confirmation_height
    }
}

impl AnchorFromBlockPosition for ConfirmationHeightAnchor {
    fn from_block_position(_block: &bitcoin::Block, block_id: BlockId, _tx_pos: usize) -> Self {
        Self {
            anchor_block: block_id,
            confirmation_height: block_id.height,
        }
    }
}

/// An [`Anchor`] implementation that also records the exact confirmation time and height of the
/// transaction.
///
/// Note that the confirmation block and the anchor block can be different here.
///
/// Refer to [`Anchor`] for more details.
#[derive(Debug, Default, Clone, PartialEq, Eq, Copy, PartialOrd, Ord, core::hash::Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Deserialize, serde::Serialize),
    serde(crate = "serde_crate")
)]
pub struct ConfirmationTimeHeightAnchor {
    /// The anchor block.
    pub anchor_block: BlockId,
    /// The confirmation height of the chain data being anchored.
    pub confirmation_height: u32,
    /// The confirmation time of the chain data being anchored.
    pub confirmation_time: u64,
}

impl Anchor for ConfirmationTimeHeightAnchor {
    fn anchor_block(&self) -> BlockId {
        self.anchor_block
    }

    fn confirmation_height_upper_bound(&self) -> u32 {
        self.confirmation_height
    }
}

impl AnchorFromBlockPosition for ConfirmationTimeHeightAnchor {
    fn from_block_position(block: &bitcoin::Block, block_id: BlockId, _tx_pos: usize) -> Self {
        Self {
            anchor_block: block_id,
            confirmation_height: block_id.height,
            confirmation_time: block.header.time as _,
        }
    }
}

/// A `TxOut` with as much data as we can retrieve about it
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FullTxOut<A: Anchor> {
    /// The location of the `TxOut`.
    pub outpoint: OutPoint,
    /// The `TxOut`.
    pub txout: TxOut,
    /// The position of the transaction in `outpoint` in the overall chain.
    pub chain_position: ChainPosition<A>,
    /// The txid and chain position of the transaction (if any) that has spent this output.
    pub spent_by: Option<(ChainPosition<A>, Txid)>,
    /// Whether this output is on a coinbase transaction.
    pub is_on_coinbase: bool,
}

impl<A: Anchor> FullTxOut<A> {
    /// Whether the `txout` is considered mature.
    ///
    /// Depending on the implementation of [`confirmation_height_upper_bound`] in [`Anchor`], this
    /// method may return false-negatives. In other words, interpreted confirmation count may be
    /// less than the actual value.
    ///
    /// [`confirmation_height_upper_bound`]: Anchor::confirmation_height_upper_bound
    pub fn is_mature(&self, tip: u32) -> bool {
        if self.is_on_coinbase {
            let tx_height = match &self.chain_position {
                ChainPosition::Confirmed(anchor) => anchor.confirmation_height_upper_bound(),
                ChainPosition::Unconfirmed(_) => {
                    debug_assert!(false, "coinbase tx can never be unconfirmed");
                    return false;
                }
            };
            let age = tip.saturating_sub(tx_height);
            if age + 1 < COINBASE_MATURITY {
                return false;
            }
        }

        true
    }

    /// Whether the utxo is/was/will be spendable with chain `tip`.
    ///
    /// This method does not take into account the lock time.
    ///
    /// Depending on the implementation of [`confirmation_height_upper_bound`] in [`Anchor`], this
    /// method may return false-negatives. In other words, interpreted confirmation count may be
    /// less than the actual value.
    ///
    /// [`confirmation_height_upper_bound`]: Anchor::confirmation_height_upper_bound
    pub fn is_confirmed_and_spendable(&self, tip: u32) -> bool {
        if !self.is_mature(tip) {
            return false;
        }

        let confirmation_height = match &self.chain_position {
            ChainPosition::Confirmed(anchor) => anchor.confirmation_height_upper_bound(),
            ChainPosition::Unconfirmed(_) => return false,
        };
        if confirmation_height > tip {
            return false;
        }

        // if the spending tx is confirmed within tip height, the txout is no longer spendable
        if let Some((ChainPosition::Confirmed(spending_anchor), _)) = &self.spent_by {
            if spending_anchor.anchor_block().height <= tip {
                return false;
            }
        }

        true
    }
}
