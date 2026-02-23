# QuickFIX verification (FIX 4.4 acceptor)

You can verify the FIX adapter with a QuickFIX initiator (or any FIX 4.4 client).

## 1. Start the server

```bash
cargo run
```

- HTTP: `http://0.0.0.0:8080` (REST + WebSocket)
- FIX: `0.0.0.0:9876` (TCP; use `FIX_PORT` to change)

## 2. QuickFIX initiator config

Create a config file (e.g. `quickfix_client.cfg`) for the **initiator**:

```ini
[default]
ConnectionType=initiator
ReconnectInterval=5
FileStorePath=store
FileLogPath=log
StartTime=00:00:00
EndTime=00:00:00
UseDataDictionary=N

[session]
BeginString=FIX.4.4
SenderCompID=CLIENT
TargetCompID=DIRED
SocketConnectHost=127.0.0.1
SocketConnectPort=9876
```

- **SenderCompID** = your client ID (acceptor expects target 56=CLIENT in our responses).
- **TargetCompID** = DIRED (our acceptor sends 49=DIRED).

## 3. Run QuickFIX initiator

Run your QuickFIX initiator with this config so it connects to `127.0.0.1:9876`. After Logon:

1. **NewOrderSingle (35=D)**  
   Send an order with ClOrdID (11), Symbol (55)=1, Side (54)=1 (Buy), OrderQty (38), OrdType (40)=2, Price (44), TimeInForce (59)=0.  
   You should receive one or more **ExecutionReport (35=8)** with ExecType (150) and OrdStatus (39).

2. **OrderCancelRequest (35=F)**  
   Send with OrigClOrdID (41) = the ClOrdID of an existing order.  
   You should receive **ExecutionReport** with OrdStatus=4 (Canceled).

3. **OrderCancelReplaceRequest (35=G)**  
   Send with OrigClOrdID (41) and new ClOrdID (11), plus updated order fields.  
   You should receive **ExecutionReport(s)** for the replacement.

## 4. Automated tests

The repo includes stub tests that do not require QuickFIX:

```bash
cargo test --test fix_adapter
```

These connect to the FIX acceptor over TCP, send a Logon and a NewOrderSingle, and assert on the ExecutionReport. Use these for CI; use QuickFIX for manual or end-to-end verification.
