use layerx_types::intent::EvmAddress;

use crate::json::Json;
use crate::rpc::{parse_url, raw_call, EndpointConfig, EndpointFailure, EndpointFault};

/// Exact 32-byte Paxeer transaction hash.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TransactionHash([u8; 32]);

/// Why a textual transaction hash could not be decoded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionHashError {
    MissingPrefix,
    WrongLength,
    NotHex,
}

impl TransactionHash {
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }

    /// Decodes a 0x-prefixed 64-digit hex hash.
    ///
    /// # Errors
    ///
    /// Returns the first prefix, length or digit defect.
    pub fn from_hex(text: &str) -> Result<Self, TransactionHashError> {
        let digits = text
            .strip_prefix("0x")
            .ok_or(TransactionHashError::MissingPrefix)?;
        if digits.len() != 64 {
            return Err(TransactionHashError::WrongLength);
        }
        let mut bytes = [0_u8; 32];
        for (slot, pair) in bytes.iter_mut().zip(digits.as_bytes().chunks_exact(2)) {
            match pair {
                &[high, low] => {
                    let upper = hex_nibble(high).ok_or(TransactionHashError::NotHex)?;
                    let lower = hex_nibble(low).ok_or(TransactionHashError::NotHex)?;
                    *slot = (upper << 4) | lower;
                }
                _ => return Err(TransactionHashError::WrongLength),
            }
        }
        Ok(Self(bytes))
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        let mut text = String::from("0x");
        for byte in self.0 {
            text.push(hex_char(byte >> 4));
            text.push(hex_char(byte & 0x0f));
        }
        text
    }
}

const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

fn hex_char(nibble: u8) -> char {
    char::from(HEX_DIGITS[usize::from(nibble & 0x0f)])
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// One canonical block position: height and hash together.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockRef {
    pub number: u64,
    pub hash: [u8; 32],
}

/// Whether the included transaction's execution succeeded or reverted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionOutcome {
    Succeeded,
    Reverted,
}

/// A transaction's inclusion as the chain currently reports it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransactionInclusion {
    pub block: BlockRef,
    pub transaction_index: u64,
    pub execution: ExecutionOutcome,
    pub deployed_contract: Option<EvmAddress>,
}

/// The complete set of states a tracked custody transaction can read back as.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionView {
    Unknown,
    Pending,
    Included(TransactionInclusion),
}

/// One contract log an included transaction emitted, exactly as reported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogRecord {
    pub address: EvmAddress,
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

/// Every configured endpoint failed the same read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EndpointError {
    pub failures: Vec<EndpointFailure>,
}

/// Why the declared endpoint configuration was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientConfigError {
    NoEndpoints,
    InvalidEndpointUrl { url: String },
    ZeroRequestTimeout { url: String },
}

/// Read-only Paxeer client over one or more declared endpoints.
#[derive(Clone, Debug)]
pub struct PaxeerClient {
    endpoints: Vec<EndpointConfig>,
}

impl PaxeerClient {
    /// Validates and adopts the declared endpoint configuration.
    ///
    /// # Errors
    ///
    /// Refuses an empty endpoint list, an unparseable URL, or a zero timeout.
    pub fn new(endpoints: Vec<EndpointConfig>) -> Result<Self, ClientConfigError> {
        if endpoints.is_empty() {
            return Err(ClientConfigError::NoEndpoints);
        }
        for endpoint in &endpoints {
            if parse_url(&endpoint.url).is_none() {
                return Err(ClientConfigError::InvalidEndpointUrl {
                    url: endpoint.url.clone(),
                });
            }
            if endpoint.request_timeout.is_zero() {
                return Err(ClientConfigError::ZeroRequestTimeout {
                    url: endpoint.url.clone(),
                });
            }
        }
        Ok(Self { endpoints })
    }

    fn read<T>(
        &self,
        failovers: &mut Vec<EndpointFailure>,
        method: &str,
        params: &[Json],
        decode: impl Fn(&Json) -> Result<T, String>,
    ) -> Result<T, EndpointError> {
        let mut failures = Vec::new();
        for endpoint in &self.endpoints {
            match raw_call(endpoint, method, params) {
                Ok(value) => match decode(&value) {
                    Ok(decoded) => {
                        failovers.append(&mut failures);
                        return Ok(decoded);
                    }
                    Err(detail) => failures.push(EndpointFailure {
                        url: endpoint.url.clone(),
                        fault: EndpointFault::UnexpectedValue { detail },
                    }),
                },
                Err(failure) => failures.push(failure),
            }
        }
        Err(EndpointError { failures })
    }

    /// Reads the current canonical head number.
    ///
    /// # Errors
    ///
    /// Returns every endpoint's typed failure when no endpoint served the read.
    pub fn head_number(&self) -> Result<u64, EndpointError> {
        self.head_number_with_failovers(&mut Vec::new())
    }

    pub(crate) fn head_number_with_failovers(
        &self,
        failovers: &mut Vec<EndpointFailure>,
    ) -> Result<u64, EndpointError> {
        self.read(failovers, "eth_blockNumber", &[], quantity)
    }

    /// Reads the canonical block at one height, if that height exists.
    ///
    /// # Errors
    ///
    /// Returns every endpoint's typed failure when no endpoint served the read.
    pub fn block_by_number(&self, number: u64) -> Result<Option<BlockRef>, EndpointError> {
        self.block_by_number_with_failovers(&mut Vec::new(), number)
    }

    pub(crate) fn block_by_number_with_failovers(
        &self,
        failovers: &mut Vec<EndpointFailure>,
        number: u64,
    ) -> Result<Option<BlockRef>, EndpointError> {
        self.read(
            failovers,
            "eth_getBlockByNumber",
            &[Json::Text(format!("0x{number:x}")), Json::Bool(false)],
            block_reference,
        )
    }

    /// Reads a custody transaction's inclusion receipt, if it is included.
    ///
    /// # Errors
    ///
    /// Returns every endpoint's typed failure when no endpoint served the read.
    pub fn transaction_receipt(
        &self,
        transaction: TransactionHash,
    ) -> Result<Option<TransactionInclusion>, EndpointError> {
        self.transaction_receipt_with_failovers(&mut Vec::new(), transaction)
    }

    /// Reads the chain identifier the endpoints agree to serve.
    ///
    /// # Errors
    ///
    /// Returns every endpoint's typed failure when no endpoint served the read.
    pub fn chain_id(&self) -> Result<u64, EndpointError> {
        self.chain_id_with_failovers(&mut Vec::new())
    }

    pub(crate) fn chain_id_with_failovers(
        &self,
        failovers: &mut Vec<EndpointFailure>,
    ) -> Result<u64, EndpointError> {
        self.read(failovers, "eth_chainId", &[], quantity)
    }

    /// Reads a custody transaction's inclusion together with every log it
    /// emitted, from one atomic receipt read.
    ///
    /// # Errors
    ///
    /// Returns every endpoint's typed failure when no endpoint served the read.
    pub fn transaction_logs(
        &self,
        transaction: TransactionHash,
    ) -> Result<Option<(TransactionInclusion, Vec<LogRecord>)>, EndpointError> {
        self.transaction_logs_with_failovers(&mut Vec::new(), transaction)
    }

    pub(crate) fn transaction_logs_with_failovers(
        &self,
        failovers: &mut Vec<EndpointFailure>,
        transaction: TransactionHash,
    ) -> Result<Option<(TransactionInclusion, Vec<LogRecord>)>, EndpointError> {
        self.read(
            failovers,
            "eth_getTransactionReceipt",
            &[Json::Text(transaction.to_hex())],
            inclusion_with_logs,
        )
    }

    pub(crate) fn transaction_receipt_with_failovers(
        &self,
        failovers: &mut Vec<EndpointFailure>,
        transaction: TransactionHash,
    ) -> Result<Option<TransactionInclusion>, EndpointError> {
        self.read(
            failovers,
            "eth_getTransactionReceipt",
            &[Json::Text(transaction.to_hex())],
            inclusion,
        )
    }

    /// Reads the full unknown-pending-included view of a custody transaction.
    ///
    /// # Errors
    ///
    /// Returns every endpoint's typed failure when no endpoint served the read.
    pub fn transaction(
        &self,
        transaction: TransactionHash,
    ) -> Result<TransactionView, EndpointError> {
        self.transaction_with_failovers(&mut Vec::new(), transaction)
    }

    pub(crate) fn transaction_with_failovers(
        &self,
        failovers: &mut Vec<EndpointFailure>,
        transaction: TransactionHash,
    ) -> Result<TransactionView, EndpointError> {
        if let Some(included) = self.transaction_receipt_with_failovers(failovers, transaction)? {
            return Ok(TransactionView::Included(included));
        }
        let known = self.read(
            failovers,
            "eth_getTransactionByHash",
            &[Json::Text(transaction.to_hex())],
            |value| Ok(!value.is_null()),
        )?;
        Ok(if known {
            TransactionView::Pending
        } else {
            TransactionView::Unknown
        })
    }
}

fn required<'a>(value: &'a Json, name: &str) -> Result<&'a Json, String> {
    value
        .member(name)
        .ok_or_else(|| format!("missing member {name}"))
}

fn quantity(value: &Json) -> Result<u64, String> {
    let text = value
        .as_text()
        .ok_or_else(|| format!("expected hex quantity, got {value:?}"))?;
    let digits = text
        .strip_prefix("0x")
        .ok_or_else(|| format!("missing 0x prefix in {text}"))?;
    if digits.is_empty() {
        return Err(format!("empty hex quantity in {text}"));
    }
    u64::from_str_radix(digits, 16).map_err(|_| format!("unparseable hex quantity {text}"))
}

fn fixed<const N: usize>(value: &Json) -> Result<[u8; N], String> {
    let text = value
        .as_text()
        .ok_or_else(|| format!("expected hex bytes, got {value:?}"))?;
    let digits = text
        .strip_prefix("0x")
        .ok_or_else(|| format!("missing 0x prefix in {text}"))?;
    if digits.len() != N.saturating_mul(2) {
        return Err(format!("expected {N} bytes, got {text}"));
    }
    let mut bytes = [0_u8; N];
    for (slot, pair) in bytes.iter_mut().zip(digits.as_bytes().chunks_exact(2)) {
        match pair {
            &[high, low] => {
                let upper = hex_nibble(high).ok_or_else(|| format!("non-hex digit in {text}"))?;
                let lower = hex_nibble(low).ok_or_else(|| format!("non-hex digit in {text}"))?;
                *slot = (upper << 4) | lower;
            }
            _ => return Err(format!("expected {N} bytes, got {text}")),
        }
    }
    Ok(bytes)
}

fn block_reference(value: &Json) -> Result<Option<BlockRef>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let number = quantity(required(value, "number")?)?;
    let hash = fixed::<32>(required(value, "hash")?)?;
    Ok(Some(BlockRef { number, hash }))
}

fn variable_bytes(value: &Json) -> Result<Vec<u8>, String> {
    let text = value
        .as_text()
        .ok_or_else(|| format!("expected hex bytes, got {value:?}"))?;
    let digits = text
        .strip_prefix("0x")
        .ok_or_else(|| format!("missing 0x prefix in {text}"))?;
    if digits.len() % 2 != 0 {
        return Err(format!("odd hex length in {text}"));
    }
    let mut bytes = Vec::with_capacity(digits.len() / 2);
    for pair in digits.as_bytes().chunks_exact(2) {
        match pair {
            &[high, low] => {
                let upper = hex_nibble(high).ok_or_else(|| format!("non-hex digit in {text}"))?;
                let lower = hex_nibble(low).ok_or_else(|| format!("non-hex digit in {text}"))?;
                bytes.push((upper << 4) | lower);
            }
            _ => return Err(format!("odd hex length in {text}")),
        }
    }
    Ok(bytes)
}

fn log_record(value: &Json) -> Result<LogRecord, String> {
    let address = EvmAddress::new(fixed::<20>(required(value, "address")?)?);
    let topics = match required(value, "topics")? {
        Json::Array(items) => items
            .iter()
            .map(fixed::<32>)
            .collect::<Result<Vec<_>, _>>()?,
        other => return Err(format!("expected topics array, got {other:?}")),
    };
    let data = variable_bytes(required(value, "data")?)?;
    Ok(LogRecord {
        address,
        topics,
        data,
    })
}

fn inclusion_with_logs(
    value: &Json,
) -> Result<Option<(TransactionInclusion, Vec<LogRecord>)>, String> {
    let Some(included) = inclusion(value)? else {
        return Ok(None);
    };
    let logs = match required(value, "logs")? {
        Json::Array(items) => items
            .iter()
            .map(log_record)
            .collect::<Result<Vec<_>, _>>()?,
        other => return Err(format!("expected logs array, got {other:?}")),
    };
    Ok(Some((included, logs)))
}

fn inclusion(value: &Json) -> Result<Option<TransactionInclusion>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let number = quantity(required(value, "blockNumber")?)?;
    let hash = fixed::<32>(required(value, "blockHash")?)?;
    let transaction_index = quantity(required(value, "transactionIndex")?)?;
    let execution = match quantity(required(value, "status")?)? {
        0 => ExecutionOutcome::Reverted,
        1 => ExecutionOutcome::Succeeded,
        other => return Err(format!("unknown execution status {other}")),
    };
    let deployed_contract = match value.member("contractAddress") {
        None | Some(Json::Null) => None,
        Some(address) => Some(EvmAddress::new(fixed::<20>(address)?)),
    };
    Ok(Some(TransactionInclusion {
        block: BlockRef { number, hash },
        transaction_index,
        execution,
        deployed_contract,
    }))
}
