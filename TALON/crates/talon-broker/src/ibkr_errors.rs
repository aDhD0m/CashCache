/// Known IBKR TWS API error codes and their meanings.
///
/// Reference: IBKR TWS API documentation, error codes section.
/// These are the most commonly encountered codes for trading operations.
pub struct IbkrErrorCode;

impl IbkrErrorCode {
    // --- Connection errors ---
    pub const CANT_CONNECT_TO_TWS: i32 = 502;
    pub const NOT_CONNECTED: i32 = 504;
    pub const CLIENT_ID_IN_USE: i32 = 326;
    pub const SOCKET_EXCEPTION: i32 = 509;
    pub const MAX_CONNECTIONS: i32 = 531;

    // --- Order errors ---
    pub const ORDER_REJECTED: i32 = 201;
    pub const ORDER_CANCELLED: i32 = 202;
    pub const SECURITY_NOT_FOUND: i32 = 200;
    pub const DUPLICATE_ORDER_ID: i32 = 103;
    pub const INVALID_ORDER_TYPE: i32 = 387;
    pub const ORDER_NOT_FOUND: i32 = 135;
    pub const ORDER_SIZE_ZERO: i32 = 110;
    pub const CROSS_SIDE_AGGR: i32 = 399;

    // --- Rate limiting ---
    pub const MAX_RATE_EXCEEDED: i32 = 100;
    pub const PACE_VIOLATION: i32 = 162;
    pub const MAX_TICKERS: i32 = 101;
    pub const MARKET_DATA_NOT_SUBSCRIBED: i32 = 354;
    pub const HISTORICAL_DATA_PACING: i32 = 366;

    // --- Account errors ---
    pub const NO_TRADING_PERMISSIONS: i32 = 460;
    pub const INSUFFICIENT_FUNDS: i32 = 201; // overlaps with ORDER_REJECTED
    pub const BUYING_POWER_EXCEEDED: i32 = 462;
    pub const PDT_RESTRICTION: i32 = 463;
    pub const ACCOUNT_NOT_FINANCIAL_ADVISOR: i32 = 321;

    // --- Market data errors ---
    pub const NO_MARKET_DATA_PERMS: i32 = 10090;
    pub const DELAYED_MARKET_DATA: i32 = 10167;
    pub const NO_SECURITY_DEFINITION: i32 = 200;
    pub const MARKET_DATA_FARM_DISCONNECTED: i32 = 2104;
    pub const MARKET_DATA_FARM_CONNECTED: i32 = 2106;
    pub const HMDS_DATA_FARM_DISCONNECTED: i32 = 2105;
    pub const HMDS_DATA_FARM_CONNECTED: i32 = 2107;

    // --- System notices ---
    pub const SYSTEM_MESSAGE: i32 = 1100;
    pub const CONNECTIVITY_LOST: i32 = 1100;
    pub const CONNECTIVITY_RESTORED: i32 = 1102;
    pub const DATA_LOST_CONNECTION: i32 = 2103;

    /// Human-readable description for an IBKR error code.
    pub fn describe(code: i32) -> &'static str {
        match code {
            100 => "Max rate of messages per second exceeded",
            101 => "Max number of simultaneous tickers exceeded",
            103 => "Duplicate order ID",
            110 => "Order size is zero",
            135 => "Order not found for cancellation",
            162 => "Historical data pacing violation",
            200 => "Security definition not found",
            201 => "Order rejected",
            202 => "Order cancelled",
            321 => "Account is not a Financial Advisor account",
            326 => "Client ID already in use",
            354 => "Not subscribed to requested market data",
            366 => "Historical data pacing violation",
            387 => "Invalid order type for this security",
            399 => "Cross side aggression limit exceeded",
            460 => "No trading permissions for this instrument",
            462 => "Buying power exceeded",
            463 => "PDT restriction — day trade count exceeded",
            502 => "Cannot connect to TWS — verify Gateway is running",
            504 => "Not connected to TWS",
            509 => "Socket exception — connection interrupted",
            531 => "Max number of connections reached",
            1100 => "Connectivity between IB and TWS lost",
            1102 => "Connectivity restored — data maintained",
            2103 => "Market data connection lost",
            2104 => "Market data farm connection inactive",
            2105 => "HMDS data farm connection inactive",
            2106 => "Market data farm connection is OK",
            2107 => "HMDS data farm connection is OK",
            10090 => "No market data permissions for this security",
            10167 => "Delayed market data — using 15-min delayed quotes",
            _ => "Unknown IBKR error code",
        }
    }

    /// Whether this is a transient error that should be retried.
    pub fn is_transient(code: i32) -> bool {
        matches!(
            code,
            100 | 162 | 366 | 502 | 504 | 509 | 1100 | 2103 | 2104 | 2105
        )
    }

    /// Whether this is a fatal error that should stop the connection.
    pub fn is_fatal(code: i32) -> bool {
        matches!(code, 326 | 531 | 460)
    }
}
