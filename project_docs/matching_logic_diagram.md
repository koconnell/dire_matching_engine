```mermaid
flowchart TD
    Start([New Order Received]) --> ValidateOrder{Valid Order?}
    
    ValidateOrder -->|No| Reject[Reject Order]
    Reject --> LogReject[Log Rejection]
    LogReject --> EndReject([Send Rejection])
    
    ValidateOrder -->|Yes| CheckSide{Order Side?}
    
    CheckSide -->|Buy| CheckSellBook{Sell Orders Available?}
    CheckSide -->|Sell| CheckBuyBook{Buy Orders Available?}
    
    CheckSellBook -->|No| AddToBuyBook[Add to Buy Side of Book]
    CheckBuyBook -->|No| AddToSellBook[Add to Sell Side of Book]
    
    AddToBuyBook --> LogNew[Log NEW Event]
    AddToSellBook --> LogNew
    LogNew --> SendAck[Send Acknowledgment]
    SendAck --> EndNew([Order Resting])
    
    CheckSellBook -->|Yes| GetBestSell[Get Best Sell Price]
    CheckBuyBook -->|Yes| GetBestBuy[Get Best Buy Price]
    
    GetBestSell --> PriceMatch{Buy Price >= Sell Price?}
    GetBestBuy --> PriceMatch
    
    PriceMatch -->|No| NoMatch[No Match Possible]
    NoMatch --> AddToBuyBook
    NoMatch --> AddToSellBook
    
    PriceMatch -->|Yes| CheckSelfTrade{Self-Trade?}
    
    CheckSelfTrade -->|Yes| SkipOrder[Skip This Order]
    SkipOrder --> GetNextOrder[Get Next Order from Book]
    GetNextOrder --> PriceMatch
    
    CheckSelfTrade -->|No| DetermineQty[Determine Match Quantity]
    
    DetermineQty --> CalcQty[Match Qty = min(Order Qty, Resting Qty)]
    CalcQty --> GenTrade[Generate Trade]
    
    GenTrade --> UpdateResting[Update Resting Order]
    UpdateResting --> UpdateIncoming[Update Incoming Order]
    
    UpdateIncoming --> CheckRestingQty{Resting Order Filled?}
    
    CheckRestingQty -->|Yes| RemoveResting[Remove from Book]
    CheckRestingQty -->|No| PartialResting[Keep in Book - Partially Filled]
    
    RemoveResting --> LogRestingFilled[Log FILLED Event - Resting]
    PartialResting --> LogRestingPartial[Log PARTIALLY_FILLED - Resting]
    
    LogRestingFilled --> SendExecResting[Send Execution Report - Resting]
    LogRestingPartial --> SendExecResting
    
    SendExecResting --> CheckIncomingQty{Incoming Order Filled?}
    
    CheckIncomingQty -->|Yes| LogIncomingFilled[Log FILLED Event - Incoming]
    CheckIncomingQty -->|No| LogIncomingPartial[Log PARTIALLY_FILLED - Incoming]
    
    LogIncomingFilled --> SendExecIncoming[Send Execution Report - Incoming]
    LogIncomingPartial --> SendExecIncoming
    
    SendExecIncoming --> CheckRemaining{Remaining Quantity?}
    
    CheckRemaining -->|Yes| ContinueMatch[Continue Matching]
    ContinueMatch --> CheckSellBook
    ContinueMatch --> CheckBuyBook
    
    CheckRemaining -->|No| PublishMD[Publish Market Data Update]
    PublishMD --> EndFilled([Match Complete])
    
    style Start fill:#e1f5ff
    style EndReject fill:#ffe1e1
    style EndNew fill:#fff4e1
    style EndFilled fill:#e1ffe1
    style GenTrade fill:#e1ffe1
```

**Matching Logic Flow**
This flowchart details the price-time priority matching algorithm including self-trade prevention and partial fill handling.
