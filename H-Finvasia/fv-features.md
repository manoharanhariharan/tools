# Finvasia / Shoonya "NorenTrader" (UFT) â€” Technology, Feed Architecture & Feature Inventory

> **Purpose of this document:** reverse-engineer the installed Finvasia trading terminal well enough to
> (a) understand where its performance bottlenecks come from and
> (b) drive a feature-parity migration onto the in-house stack (**Tauri 2 + Rust workspace + Svelte 5**).
>
> Every claim below is tagged:
> **[Observed]** = read directly from shipped files, metadata, logs or documentation on this machine.
> **[Inferred]** = reasoned from observed evidence, not directly proven.
> **[Unknown]** = not recoverable without runtime instrumentation or vendor assets.
>
> Analysis is static. No part of the application was executed.

---

## 1. Target identity

| Item | Value | Evidence |
|---|---|---|
| Product | Finvasia Live (Shoonya) trading terminal; internally **"UFT" / "Noren Trader"** | window/type names `UFTrader.*`, `NorenTrader.exe.config` [Observed] |
| Installer | Squirrel.Windows (delta packages, `Update.exe`) | `packages/*.nupkg`, `RELEASES`, `Squirrel.dll` [Observed] |
| Installed build | `app-2025.1.14` | folder name, `RELEASES` [Observed] |
| Main assembly | `Finvasia.exe` â€” .NET assembly name **NorenTrader**, file ver `1.0.8`, product ver `1.2.0` | PE metadata [Observed] |
| Signer / vendor | **KAMBALA SOLUTIONS PRIVATE LIMITED** | `FileVersionInfo.CompanyName` [Observed] |
| Manual copyright | "Â© 2022 Kambala Solutions Pvt Ltd" | `doc/uft_document.chm` [Observed] |
| Intended market | NSE / BSE / NFO / BFO / MCX / CDS; dealer + investor roles | config, logs, manual [Observed] |
| Runtime footprint | 1 process, ~7.6 MB entry EXE + ~90 DLLs | file listing [Observed] |

### 1.1 Code size (from PE/CLI metadata)

| Assembly | Types | Methods | Role |
|---|---:|---:|---|
| `Finvasia.exe` (NorenTrader) | 1,330 | 24,308 | app shell, all WPF windows/view-models, stores, DB |
| `NorenPlugin.dll` | 465 | 21,358 | **all wire messages (`OmsProtos.*`)** + plugin SDK + shared controls |
| `NorenApi.dll` | 107 | 709 | broker-channel API layer (Feed/Order/Req/TouchLine) |
| `CozyMQ.dll` | 32 | 182 | **socket message bus** (transport + framing + auth) |
| `CozyApi.NorenConnection` | â€“ | â€“ | device identity + license handshake |
| `NorenBluApi.dll` | 35 | 181 | BLU (bulk/line order) API |
| `NorenRTD.dll` | 42 | 157 | Excel RTD server |
| `NewsApi.dll` | 14 | 38 | news feed client |
| `Plugins/Charts.dll` | 57 | 457 | desktop charts + VWAP |
| `Plugins/EquitySipPlugin.dll` | 49 | 671 | equity SIP |
| `Plugins/MutualFundPlugin.dll` | 74 | 1,471 | mutual funds |

**This is a large, mature, single-process WPF monolith â€” not Electron and not a web app.**

---

## 2. Technology stack

### 2.1 Platform & UI

| Layer | Technology | Notes |
|---|---|---|
| Runtime | **.NET Framework 4.6.2**, AnyCPU (x86/x64 interop DLLs shipped) | `Finvasia.exe.config` `<supportedRuntime>` [Observed] |
| Language | C# (with WPF XAML), F# helper lib present (`FSharp.Core`) | [Observed] |
| UI toolkit | **WPF** + **DevExpress XPF v20.2** (grids, docking, ribbon, charts, gauges, printing, RichEdit, PDF) | `DevExpress.*.v20.2.dll` Ã—20 [Observed] |
| Theming | DevExpress `DXStyle`, `Office2019Black`, `Office2019Colorful` | [Observed] |
| Embedded browser | **Microsoft WebView2** (Chromium/Edge) for web charts & disclaimers | `Microsoft.Web.WebView2.*`, `Finvasia.exe.WebView2/EBWebView` [Observed] |
| Icons | `FontAwesome.WPF` | [Observed] |
| Installer | Squirrel.Windows + `DeltaCompressionDotNet`, `SharpCompress` | [Observed] |

### 2.2 Data & serialization

| Concern | Technology | Notes |
|---|---|---|
| **Wire serialization** | **protobuf-net** (`ProtoBuf`) | 317 `[ProtoContract]` types / **5,485 `[ProtoMember]` fields** [Observed] |
| Message model | `OmsProtos.*` namespace, ~300 message types | [Observed] |
| JSON | `System.Text.Json` + custom `NorenJsonUtil` | settings, scrip master, workspaces [Observed] |
| Local RDBMS | **SQLite** via `System.Data.SQLite` + **Entity Framework 6** | `SqlLiteDb`, `ScripsDb`, `ClientsDb` [Observed] |
| Crypto | `BouncyCastle.Crypto`; CozyMQ ships `AesGcm256` + `BCSHA256` | [Observed] |
| Excel/math | `ExcelFinancialFunctions` | option greeks / payoff [Observed] |
| QR / 2FA | `QRCoder` | TOTP/QR login [Observed] |
| Assembly rewriting | `Mono.Cecil` | property-grid/filter patching at boot [Inferred] |

### 2.3 Networking

| Concern | Technology |
|---|---|
| Market data + orders + requests | **Custom pub/sub message bus `CozyMQ` over raw TCP** (TLS-capable), not HTTP, not WebSocket |
| Transport features | LZ4 compression, AES-GCM-256, trie-based subscription keys, wildcard `*`, forward-all / round-robin, handshake, keep-alive, auto-reconnect |
| Web content | `HttpClient` to chart/news endpoints; WebView2 |
| Excel bridge | Named Pipes (`UFTrader.NamedPipeServer`) + COM RTD (`NorenRTD`) + XLL add-in (`AddIns/NorenLink.xll`) |

---

## 3. How live market data is actually fetched

This is the part that matters most for a rewrite.

### 3.1 It is not REST polling and not a WebSocket

The terminal opens **six separate persistent TCP sockets** to the broker at login. Each socket is a
`CozyMQ` "device" with its own purpose and port. Ports observed in a real session log:

```
08:58:00 COZY  UFT tcp://tws.shoonya.com:56005 is disposed
08:58:00 COZY  UFT:FN131465 starts with tcp://tws.shoonya.com:56001
08:58:00 COZY  REQUEST  tcp://tws.shoonya.com:5809 is connected
08:58:00 COZY  TCHLINE  tcp://tws.shoonya.com:5813 is connected
08:58:00 COZY  ORDER    tcp://tws.shoonya.com:5801 is connected
08:58:00 COZY  FEED     tcp://tws.shoonya.com:5817 is connected
```
(source: `%APPDATA%\NorenTrader\<endpoint>_<key>_<user>\SessionLogs\*.txt`)

| Logical channel | Default port | Purpose | Code owner |
|---|---:|---|---|
| **UFT / control** | 56005 | session + config bootstrap (`NORENUI_CONNECTION_ENDPOINT`, key `FINLIVN_2FA`) | `CozyApi.NorenConnection` |
| **UFT app** | 56001 | app-level device registration, global-IP/SSL negotiation | `NorenApiMain` |
| **FEED** | 5817 | market-watch tick stream, IV feed, auction feed, lagged feed | `NorenApi.FeedBroker` |
| **TCHLINE** | 5813 | touchline / depth (5-level & 20-level) stream | `NorenApi.TouchLineBroker` |
| **ORDER** | 5801 | order entry, modify, cancel, square-off, order/trade push | `NorenApi.OrderBroker` |
| **REQUEST** | 5809 | request/response RPC: login, limits, positions, holdings, masters, admin | `NorenApi.ReqBroker` |

Splitting channels this way is deliberate: a burst of ticks on FEED cannot delay an order on ORDER.

### 3.2 The protocol on the wire

```
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”   1. TCP connect (plain tcp:// or ssl://)      â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚  Client  â”‚ â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â–¶  â”‚  CozyMQ bus â”‚
â”‚ CozyMQ   â”‚   2. handshake + device identity               â”‚   broker    â”‚
â”‚          â”‚   3. license check (CMD_LICENSE_CHECK)         â”‚             â”‚
â”‚          â”‚   4. SUB <trie.key> / R REG / R HND            â”‚             â”‚
â”‚          â”‚ â—€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€   â”‚             â”‚
â”‚          â”‚   5. framed messages: control char + payload   â”‚             â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜                                                â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
                          payload = protobuf-net(OmsProtos.*)
```

**Framing / control plane** (string constants recovered from `CozyMQ.dll`) [Observed]:

- control messages: `R SUB`, `R UNSUB`, `R REG`, `R UNREG`, `R HND`, `R UNHND` (request/response
  variants are distinguished by control chars `COZY_CONTROL_MESSAGE_REQUEST/RESPONSE/ENDRESPONSE/ELSEMESSAGE/HANDOVERMSG`).
- subscription keys are **trie paths** joined by `COZY_TRIE_SUB_DELIMETER`, with a wildcard
  `COZY_TRIE_ANY_CRITERIA` (`*`).
- forwarding modes: `fwd_all`, `round_robin`.
- compression / crypto markers: `COZY_ENCRYPT_TYPE_LZ4_COMPRESS`, `AesGcm256`, `BCSHA256`.
- bounded buffers: `COZY_DEVICE_READBUFFER_LEN`, `COZY_PAYLOAD_MAX`, `COZY_NUM_512_K`, `COZY_NUM_1_K`.

**Application keys** are *not* opaque integers â€” they are short ASCII codes prefixed on the payload.
Recovered from `NorenApi.dll` [Observed], e.g.:
`swPlcOrd` (place), `swModOrd`, `swCanOrd`, `swSqrOff`, `crAuthUsr` (login), `crGtPrms` (limits),
`crGtHolInf` (holdings), `crGtProflst`, `whiOrdGtHsty` (order history), `smGtInf` (scrip info),
`OSW_UPD` / `OSW_FILL` / `OSW_ANC` (order/trade push), `EXCH_MSG`, `ADMIN_MSG`, `REQ_LMT`, `UI_MSG`.

**Payload types** [Observed] â€” key market-data messages in `OmsProtos`:

| Message | Meaning |
|---|---|
| `MktFeed`, `Marketfeed`, `MktScrip`, `Scripupdate` | LTP / volume / OHLC / OI tick |
| `DepthLevel`, `Depth20`, `ExtendedDepth` | 5-level and 20-level book |
| `OhlcObj`, `Impvol`, `Indexdata` | OHLC blobs, implied vol, indices |
| `StreamMsg`, `MessageStreamAlerts` | streamed alert envelopes |
| `Scripdetails`, `CompanyData`, `DerivativeAnalysisItem` | fundamentals / analytics |
| `OrderAck`, `TradeMsg`, `Ack`, `Limits`, `Holdings`, `Positions*` | order & account state |
| `MagBasket*`, `BasketOrder*`, `AlgoExecutor*`, `BluBroadcast*`, `SipBasket` | basket/algo/BLU/SIP products |

### 3.3 Subscription lifecycle

1. A scrip is added to a market-watch group â‡’ `FeedBroker.SubscribeFor_MW_Update(exch, token, bool)`
   and (for depth) `TouchLineBroker.SubscribeFor_DEPTH_Update(...)`.
2. Sectional extras attach themselves: `SubscribeFor_IV_Updates`, `AddExchStatusSubscription`,
   `AddExchMessageSubscription`, `AddAuctionFeedSubscription`, `AddLaggedFeedSubscription`.
3. Active subscriptions are tracked in a "sub store" keyed by scrip, so duplicates are suppressed
   (`NorenDeviceCb.IsAlreadySubbed`, `AddToSubStore`, `FindAndRemove`).
4. On reconnect (`OnBeginReconnect` â†’ `OnReconnected`) the client **re-sends every subscription and
   registration** (`ReconSubscriptions`, `ReconRegistration`) and can fail over to a backup server
   (`SwitchToBackup`, `ConnectToBackupServer`).
5. A broker-side cap applies. Observed log: `Max Watch Scrips Limit : 30` and
   `Max Watch Scrips Sub Limit reached`. [Observed]

### 3.4 Client-side fan-out into the UI

```
CozyMQ socket thread
   â””â”€ CozyDeviceCallback.Process()                 // decode frames
        â””â”€ NorenApi.NorenDeviceCb.OnElseMsgNotify  // route by key
             â”œâ”€ FeedBroker / TouchLineBroker       // MW & depth
             â”œâ”€ OrderBroker                        // order/trade pushes
             â””â”€ ReqBroker                          // request responses
                  â””â”€ NorenApiNotify.On*Response     // typed callbacks
                       â””â”€ MarketWatchStore.UpdateScripInfo()
                            â”œâ”€ FeedScripWatcher / DepthScripWatcher   // dedupe + merge
                            â””â”€ MTObservableCollection<T>.RaisePropertyChanged
                                 â””â”€ DevExpress GridControl (WPF binding)
```

`MTObservableCollection<T>` + `SequencingService` exist specifically to marshal multi-threaded tick
mutation onto the UI thread without throwing. [Observed types; threading detail Inferred]

---

## 4. Local storage & runtime footprint

Root: `%APPDATA%\NorenTrader\` (per-endpoint/per-user subfolder) [Observed]

```
AppSettings.json                    last user, theme, device-remember flag
<endpoint>_<key>_<user>\
  ScripsData\SD_CS06\ScripsData.json   90.8 MB  â† full instrument master (~131k scrips)
  UserSettings\US_CS01\UserSettings.json  workspace/grid/colour/order prefs
  UFT_Workspace\WS_CS01\Workspace1.xml    dock layout (DevExpress, ~430 KB)
  UFT_Workspace\BarLayout.json, rbnSettings.xml, LocData.json
  MutualFundLayout\*.xml, VWapPlugin\VwapLayOut.xml
  SessionLogs\SessionLog*.txt             1 MB â€“ 10.5 MB per session  â† see Â§6.1
```

Scrip-master counts recorded in user settings [Observed]:

| Exchange | Scrips |
|---|---:|
| NFO | 94,225 |
| NSE | 13,071 |
| BSE | 12,748 |
| BSTAR | 10,446 |
| BFO | 582 |
| **Total** | **â‰ˆ131,072** |

Every scrip record carries ~35 fields (ISIN, lot size, tick size, price precision, expiry, strike,
option type, underlying token, spread legs, SIP min/max â€¦).

---

## 5. Feature inventory (migration scope)

Derived from the shipped manual (`uft_document.chm`, 20 chapters) **cross-checked against actual
window/view-model/API names in the binaries**, so this is not just marketing copy.

Legend â€” **P0** = required for a tradeable MVP Â· **P1** = expected by active traders Â· **P2** = niche/dealer Â· **P3** = defer.

### 5.1 Market data & discovery

| # | Feature | What it does | Internals | Pri |
|---|---|---|---|---|
| M1 | **Market Watch (multi-group)** | multiple named watch groups, predefined + user groups, add/remove/reorder scrips, column chooser, separators | `MarketWatch`, `MarketWatchStore`, `PreDefGrpItem` | P0 |
| M2 | **Quick scrip search** | instant symbol/ISIN/company search with expand + near-symbol matching | `ScripsDb.ExecuteSymbolSearchQuery / ExecuteIsinSearchQuery / FindNearSymbol` | P0 |
| M3 | **Snap Quote** (`F6`, `Alt+F6`) | detailed quote panel, customisable fields, compact mode | `SnapQuoteWindow/VM`, `SnapQInstStore` | P0 |
| M4 | **Market depth (5 / 20 level)** | bid/ask ladder, order-book pressure | `MarketDepth`, `_20DepthWindow`, `DepthLevel`, `Depth20` | P1 |
| M5 | **Floating scrip view / favourites** | always-on-top mini quotes, persists when minimised | `FavScripWindow`, `FavScripStore` | P1 |
| M6 | **Index watch + spot-rate bar + title-bar indices** | index list, floating spot rates, add to title bar | `IndexWatch`, `SpotRates`, `IndxBarCtrl` | P1 |
| M7 | **Option chain** (`Ctrl+Shift+O`) | chain by expiry/strike with greeks & OI | `OptionChainWin`, `OptionChainViewModel`, `OpChainItem` | P0 |
| M8 | **Derivatives chain / implied vol / FOI snap** | futures+options ladder, IV table, FII/OI view | `DerChainWin`, `ImpVolWindow`, `FOI_SnapQuoteWin` | P2 |
| M9 | **Top-N movers** | gainers/losers by LTP, volume, value; EQ/BL and FO groups | `TopN`, `topNConstants` | P1 |
| M10 | **VWAP stats** | per-scrip VWAP panel | `VwapControl`, `VwapViewModel` | P2 |
| M11 | **Pivot levels** | classic pivot point table | `ScripPivotsWin` | P2 |
| M12 | **Scanners** | server-defined scanner groups/lists | `ScannersWin`, `ScannersViewModel` | P2 |
| M13 | **Market / exchange status** | exchange open/close & segment status | `MarketStatusWin`, `ExchStatusInfo` | P1 |
| M14 | **Exchange messages** | live broadcast ticker from exchanges | `ExchMessageWin`, `EXCH_MSG` key | P1 |
| M15 | **News** | broker news feed + popup | `NewsWin`, `NewsPopup`, `NewsApi.dll` | P2 |

### 5.2 Order management

| # | Feature | What it does | Internals | Pri |
|---|---|---|---|---|
| O1 | **Normal order entry** (`F1` buy / `F2` sell) | LIMIT / MARKET / SL-L / SL-M; product & duration; disclosed qty; remarks; inline depth; amountâ†’qty calc; order-margin check | `OrderEntry`, `OrderEntryViewModel`, `PlaceOrderParams` | P0 |
| O2 | **Products** | CNC, MIS, NRML, MTF (cash); MIS, NRML (F&O); CO, BO | `ProductLib`, `UI_*`/`LIB_*` constants | P0 |
| O3 | **AMO** (`Ctrl+F1/F2`) | after-market orders with in-window indicator | `AMO` consts, `PlaceOrderParams` | P1 |
| O4 | **Cover Order** (`Shift+F1/F2`) with trailing SL | entry + mandatory SL, trail | `CoverOrderEntry`, `CoverOrderEntryViewModel` | P1 |
| O5 | **Bracket Order** (`Shift+F3/F4`) with trailing SL | entry + target + SL | `BracketOrderEntry(ViewModel)` | P1 |
| O6 | **Trigger / GTT orders** (`Ctrl+Shift+F1` multi-leg) | condition-based orders, day or till-triggered | `TriggerOrderEntryWin`, `TriggerOrderBookViewModel` | P1 |
| O7 | **SOR (smart order routing)** (`Ctrl+F3/F4`) | price-best routing across venues | `SorOrderEntry`, `SorOrderEntryViewModel`, `SorOrderParams` | P2 |
| O8 | **Basket orders** (`Ctrl+B`) | create/place baskets; import/export to file; AMO baskets; spread baskets; rebalance; recalc | `BasketOrderWindow`, `SpreadBasketOrderWindow`, `BasketOrderParams` | P1 |
| O9 | **Slicing** | split one order into N slices or by qty, then fire | `SliceOrderWindow`, `ImmediateSliceOrder`, `OrdSlicer` | P1 |
| O10 | **Multi-leg order** (`Ctrl+Shift+F1`) + ML order book (`Ctrl+Shift+F3`) | leg-based strategies | `MultiLegOrderEntry`, `MLCompOrdBook` | P2 |
| O11 | **OFS order entry/book** | OFS (offer for sale) workflow | `OFSOrderEntry`, `OFSBook` | P3 |
| O12 | **Block / negotiated trades (CNT)** | block order entry + CNT order & trade books | `BlockOrderEntry`, `CNTOrderBook`, `CNTTradeBook` | P3 |
| O13 | **Iceberg & option-straddle algos** | server-side algo entry, algo order book, assign algo | `AlgoIceBergOrderEntry`, `OptionStraddleWin`, `AlgoOrderBook`, `AlgoExecutor*` | P2 |
| O14 | **TPIN validation** | exchange TPIN prompt for sell | `TPinValidationwin` | P2 |
| O15 | **Order authorisation / accept-reject** | dealer-maker approval flow | `OrderAuthWindow`, `AcceptRejectViewModel` | P3 |

### 5.3 Books, positions & P&L

| # | Feature | Details | Internals | Pri |
|---|---|---|---|---|
| B1 | **Order book** (`F3`) | split open/closed; 29 documented columns (Noren order id, ExchOrderID, status, fill qty, open qty, trigger, product, duration, user, client, rej reason, remarks, stop/profit/trail price, SNO order id, LocId, state) | `OrderBook`, `OpenOrdBook`, `CompOrdBook` | P0 |
| B2 | **Trade book** (`F8`) | fill id, exch order id, qty, fill price, user, client, report type | `FillBook`, `TradeMsg` | P0 |
| B3 | **Positions** (`F11`) | 22 documented fields incl. day/CF buy-sell qty & amount, avg prices, LTP, MTOM, P&L | `PositionsWindow`, `PositionsItem` | P0 |
| B4 | **Square-off** | multi-select square-off per position | `SendSqrOffOrder` | P0 |
| B5 | **Net position book** (`Alt+F6`) | real-time dealer net positions | `DealerNetPositionsWindow`, `DlrNetPosViewModel` | P2 |
| B6 | **Order history** | per-order audit trail | `OrderHistoryWindow`, `whiOrdGtHsty` | P1 |
| B7 | **Product conversion** (`Ctrl+Shift+â€¦`) + conversion report | MISâ†”CNC/NRML | `PositionConversion`, `ProductConversionBook` | P1 |
| B8 | **Holdings** (`Alt+F9`) | demat holdings; dealer view is read-only | `RedViewHoldings`, `ScripwiseHoldings` | P0 |
| B9 | **Limits / margins** (`Ctrl+Shift+V` view, `Ctrl+Shift+U` update) | limits, margin used, margin available | `RedViewLimitWindow`, `RedSetLimitsWindow`, `SingleViewLimitWin` | P0 |
| B10 | **Order margin / basket margin / span** (`Ctrl+Shift+L`) | pre-trade margin check, basket & SPAN calculator | `SpanCalculator`, `NOREN_REQ_GET_ORDER_MARGIN`, `npcCalSpan` | P1 |
| B11 | **Add holdings / T1 holdings / non-POA** | manual holding credit flows | `AddHoldingsWindow`, `AddT1HoldingsWindow`, `AddNonPOAHoldings` | P2 |
| B12 | **Report generator** (`Ctrl+Shift+R`) | parameterised reports, CSV export | `ReportGeneratorWindow`, `CsvExporter` | P2 |

### 5.4 Analytics & charting

| # | Feature | Details | Pri |
|---|---|---|---|
| A1 | **Desktop charts** (`Shift+C`) | DevExpress chart with 19 indicators: SMA, EMA, TEMA, TRIX(MA), WMA, Median/Typical/Weighted-Close price, ATR, CCI, DPO, Mass Index, MACD, ROC, RSI, StdDev, Chaikin Volatility, Williams %R, Linear Regression | P1 |
| A2 | **Web charts** | TradingView / ChartIQ via WebView2 (`NORENUI_WEBCHART_ENDPOINT`, `chartiq.html`, `PREF_CHART_IQ`) | P1 |
| A3 | **Option greeks & payoff** | Black-Scholes greeks, payoff graph, sum payoff, Greek neutralizer, vol smile | P1 |
| A4 | **Option calculator / yield calculator / issue-cap calculator** | utility calculators | P2 |
| A5 | **VWAP / volume-history / price-history** | historical series | P3 |
| A6 | **Margin basket / theme basket (MAG)** | create/modify/rebalance/recalc theme baskets | P3 |

### 5.5 Alerts & notifications

| # | Feature | Details | Pri |
|---|---|---|---|
| N1 | **Scrip alerts with GTT** | condition builder, day-valid or till-triggered, alert window | P1 |
| N2 | **Position alerts** | server push/pop position alerts | P2 |
| N3 | **Circuit-limit alerts** | price-band breach notice | P3 |
| N4 | **UI messages** | research calls, admin messages, trading calls (`UI_MSG`) | P1 |
| N5 | **Toasts + sound** | per-message-type toast/sound config | P1 |
| N6 | **Notification centre** | aggregated notification window | P1 |
| N7 | **Multi-popup place order** | popup that can place an order inline | P3 |

### 5.6 Account, login & dealer/admin (RMS)

| # | Feature | Details | Pri |
|---|---|---|---|
| S1 | **Login with 2FA/TOTP** | user id + PIN/password + OTP/TOTP; QR-based auth; password reset/forgot flow | P0 |
| S2 | **Multi-account / client switching** | dealer acts on behalf of clients, account groups, default client id | P1 |
| S3 | **App lock** (`Ctrl+W`) | PIN re-entry, lock interval pref | P1 |
| S4 | **Change password** (`Ctrl+Shift+F9`) | | P1 |
| S5 | **Entity manager** (`Ctrl+E`) | users, dealers, accounts, investors, roles, regions, brokers CRUD | P3 |
| S6 | **Role manager / online users / client sets** | entitlement management | P3 |
| S7 | **RMS: exclusion list, auto square-off, expiry square-off, re-generate orders, SNO close** | risk controls | P3 |
| S8 | **RMS: risk blocks, scrip baskets (margin/allowed/restricted), scrip groups & rules** | dealer risk config | P3 |
| S9 | **RMS: broker-level funds, BOD allocation, T1 margin drop, online allocation** | fund allocation console | P3 |
| S10 | **RMS violations / OTR report / eDIS (REP) report** | compliance artefacts | P2 |
| S11 | **BLU (basket-line/underline bulk execution)** | line book, underline, IOI book, manual fills, aggregated fills, spread, upload, reassign, colour customisation | P3 |
| S12 | **IVR call session** | outbound call workflow | P3 |

### 5.7 Wrappers / integrations

| # | Feature | Details | Pri |
|---|---|---|---|
| X1 | **Excel link (NorenLink)** | XLL add-in (`AddIns/NorenLink.xll`) + RTD server (`NorenRTD.dll`) + named-pipe transport; quote functions and order push from Excel | P2 |
| X2 | **Plugin SDK** | `Plugins/*.dll`, `IUFTPlugin` + `INorenApi`/`IUFTActions`/`IOrderApi`/`IReqApi`; ribbon menu, shortcut, `LoadonLogin`, `HideInRibbon`, plugin settings store | P3 |
| X3 | **Workspaces** | save/apply named dock layouts (`Workspace1.xml`) | P1 |
| X4 | **UI preferences** (`Ctrl+P`) | themes, fonts, title-bar items, floating windows, order defaults per exchange, colours, notify, workspace | P0 |
| X5 | **Shortcut manager** | user-rebindable hotkeys (`UFTShortcuts`, `ShortcutsMgrWin`) | P1 |
| X6 | **CSV export / import** | books & baskets | P1 |
| X7 | **Web links window** | external exchange site links (`WebLinksData.xml`) | P3 |
| X8 | **Diagnostics / debug console / perf monitor (`F12` logs)** | counters, socket meters, memory info | P1 |
| X9 | **Self-update** | Squirrel delta updates (`UpdateManager`) | P0 |

### 5.8 SIP / mutual funds (plugin-driven)

| # | Feature | Details | Pri |
|---|---|---|---|
| P1 | **Equity SIP** | daily/weekly/monthly baskets registered as SIP; SIP book; create/close/open/history | P2 |
| P2 | **Mutual funds** | MF watch groups, search, purchase, redeem, SIP entry, order books (normal + SIP), holdings, mandate id | P2 |

---

## 6. Performance bottlenecks â€” evidence and root causes

### 6.1 Exchange-message flood (largest single hot spot) â€” **[Observed]**

Log-channel census from two real sessions:

| Session | Total lines | `EXCH MSG` lines | Share |
|---|---:|---:|---:|
| 2025-10-14 | 58,642 | 57,840 | **98.6 %** |
| 2025-10-23 | 64,595 | 63,653 | **98.5 %** |

The stream is dominated by per-scrip price-band notices, e.g.
`MCX : DPL Update - The Daily Price Limit for NATGASMINI 24Oct2025 260.0 PE has been updated to 0.05 - 18.25`
â€” often the *same* contract at sub-minute intervals.

**Root cause:** every `EXCH_MSG` is written synchronously to a session log file, forwarded to the
`ExchMessageWin` view, *and* themed as an alert. Cost scales with message volume, on the UI thread
path, with no backpressure or coalescing.
**Migration fix:** bounded ring buffer per channel; batch flush every 250 ms; virtualise the viewer;
coalesce DPL updates to last-value-wins per (exch, symbol).

### 6.2 90.8 MB single-line JSON instrument master â€” **[Observed file / Inferred parse]**

`ScripsData.json` = one JSON **array** (one physical line) of â‰ˆ131,072 objects Ã— ~35 fields.
Loading it means: read 90.8 MB â†’ `JsonSerializer` materialise ~131k POCOs â†’ build lookup
dictionaries. That is hundreds of MB of transient allocation and seconds of paused startup, every
cold start, and it is re-validated against per-exchange `UpdTime`/`ScripCount` stamps.
**Migration fix:** ship a binary master (framework-coded / bincode / FlatBuffers), or SQLite with
indexes on `(exchange, token)`, `unique_key`, `symbol`, `isin`; memory-map it; load lazily per
exchange. Expected: startup tens of seconds â†’ **<1 s**.

### 6.3 One UI thread for everything â€” **[Observed architecture]**

WPF + DevExpress `GridControl` bound directly to `ObservableCollection` tick objects. `MTObservableCollection`
and `SequencingService` demonstrate the authors marshalling concurrent tick writes back to the
dispatcher. Every property change raises `PropertyChanged` â†’ grid cell invalidation â†’ layout/measure.
**Migration fix:** decouple ingest from render. Rust actor per channel â†’ coalescing store (latest
value per scrip, 30â€“60 Hz) â†’ push **deltas** to UI; render with virtualised rows
(tanstack-virtual / AG Grid) and repaint only visible cells.

### 6.4 Per-scrip subscription churn â€” **[Observed]**

`SessionLog20251106T160510.txt` shows an option-chain open producing, *per strike*, an IV
subscription **+** a touchline subscription **+** a `Get Scrips Info` request:

```
04:04:18.5040 PM,FEED,   Subscribed for IV Scrip: 40123|NFO
04:04:18.5040 PM,TCHLINE,Subscribed for Broadcast 40123|NFO
04:04:18.5040 PM,REQUEST,Request sent for Get Scrips Info : NFO|40123
04:04:18.5040 PM,INFO,   Max Watch Scrips Sub Limit reached
```

A full chain = hundreds of round trips; the broker cap (observed 30) is hit almost immediately, and
the UI shows a truncated chain. Each subscribe also allocates in the sub-store (linear `FindAndRemove`
scan per removal).
**Migration fix:** batch subscribe (`SUB` accepts `string[]`), diff-and-commit watch sets on group
change instead of per-scrip calls, cache scrip info locally from the master, and surface a clear
"subscription budget" indicator instead of silently failing.

### 6.5 Unbounded in-memory books & logs â€” **[Inferred from types]**

`OrdersStore`, `TradesStore`, `CNTOrdersStore`, `MultiLeg`/`OFS` stores keep the whole day in memory
with no eviction; `SessionLogs` grow to 10.5 MB **per session** as plain text (retained indefinitely in
`SessionLogs/`). Multi-day uptime â‡’ monotonic memory growth; no rotation policy observed.
**Migration fix:** bounded structures, ring buffer for logs, rotation + retention, structured
`tracing` with sampling instead of full-text echo.

### 6.6 Synchronous JSON settings I/O per preference â€” **[Inferred]**

`AppSettings.json`, `UserSettings.json` (per-grid keys `GRID_*`), `ColumnProfileLog.txt` (41 KB of
appended profile events) are rewritten through plain file I/O, and settings are keyed per
grid/column. Column chooser / workspace save touches many small files.
**Migration fix:** single typed config store, debounced atomic writes, one coalesced document.

### 6.7 Legacy runtime ceilings â€” **[Observed]**

.NET Framework 4.6.2 (2016) with WPF: no modern JIT/SIMD improvements, no `Span`-first parsing,
`gen0` GC pressure from `string`-heavy JSON and per-tick boxing. DevExpress **v20.2** is six major
versions behind. Any rewrite gets these wins for free.

### 6.8 Additional structural notes

- Startup performs assembly rewriting (`Mono.Cecil` shipped) and plugin scanning from `Plugins/`,
  plus `LoadLinkToExcelAddin.exe` / `LinkToExcelInstaller.exe` side processes.
- `Assets/Tools/Profiler/RefLayout.xml` is **1.18 MB** of reference layout loaded by
  `ColumnProfileApp.exe` for column profiling â€” a sign that grid layout is a known pain point.
- Six sockets each with independent reconnect + re-subscribe storms can produce a thundering-herd on
  reconnect. **[Inferred]** Backoff/jitter was not observed; `Reconnect` strings exist but intervals
  are not static-readable.

---

## 7. Migration blueprint (target: in-house Tauri 2 + Rust + Svelte 5)

The house pattern already in `H-Remote` (Tauri 2 shell + `hr-*` Rust workspace crates + Svelte 5/Vite
UI + Fluent i18n + `tracing`) fits this product well: the performance-critical part is a socket
ingest + state machine, not UI.

### 7.1 Proposed crate layout

```
crates/
  fv-core          domain types, Paise money type, SymbolKey, errors, clocks
  fv-feed          Shoonya WS protocol: frame parsing, diff-merge, subscribe batching,
                   tokio-tungstenite transport + offline replay source
  fv-master        instrument master (Mongo-backed, memory-indexed) + search
  fv-store         MongoDB repositories: snapshots, trackers, orders, settings
  fv-engine        coalescing tick store, submission-budget planner, watch-set diffing
  fv-market        REST client for the few things Mongo cannot answer
  fv-analytics     greeks, payoff, VWAP, pivots, margin/span
  fv-bridge        Tauri commands + batched delta event channel
src-tauri/         thin shell: window mgmt, commands, global shortcuts, updater
ui/                SvelteKit/Svelte 5 + Vite; virtualised grids
```

> **Replaces the earlier CozyMQ plan.** `fv-protocol` (protobuf/CozyMQ reimplementation) is dropped.
> It was blocked on 5,485 unrecoverable field numbers and a vendor licence question; the official
> Shoonya WebSocket removes both problems. See Â§7.6 and Â§12.

### 7.2 Technology mapping

| Legacy | Replace with | Why |
|---|---|---|
| .NET Framework 4.6.2 | Rust 1.85+ (stable, `rust-version` pinned) | no GC pauses, `Span`, zero-copy parsing |
| WPF + DevExpress XPF 20.2 | Svelte 5 + Vite; virtualised grid (`tanstack-virtual`, AG Grid Community) | remove licensing + dispatcher coupling |
| DevExpress charts (19 indicators) | `lightweight-charts` (TradingView OSS) + Rust indicator crate | already matches the web-chart UX they ship |
| WebView2 host | Tauri webview (same engine, no separate host process) | one less runtime |
| protobuf-net + `OmsProtos` | `serde` + typed frame enums (JSON, diff-merged) | documented contract, no schema recovery needed |
| CozyMQ client | `tokio-tungstenite` over `wss://` | standard, TLS by default, one connection |
| `System.Text.Json` @ 90.8 MB | **MongoDB `INSTRUMENT` / `FINVASIA_SYMBOL` already local** + in-process index | no 90 MB parse at all; see Â§7.9 |
| SQLite + EF6 | MongoDB (existing local instance) + explicit repository layer | data already exists; see Â§7.9 |
| `MTObservableCollection` + `SequencingService` | coalescing tick store + 30â€“60 Hz delta frame | batch instead of per-tick dispatch |
| `tracing`-free text logs | `tracing` + `tracing-appender` + rolling retention | structured, sampled |
| Squirrel.Windows | Tauri updater (`tauri-plugin-updater`) | signed delta updates |
| `NamedPipeServer` + RTD/XLL | keep a narrow named-pipe bridge; Excel add-in as separate optional deliverable | P2/P3 |
| Plugin SDK (`IUFTPlugin`) | Rust `trait FvPlugin` + optional WASM plugins | sandboxed, versioned |
| DevExpress docking | `dockview` / `golden-layout` | save/restore layouts as JSON |

### 7.3 Hard problems to plan for

1. **~~Wire-schema recovery~~ â€” no longer blocking.** Replaced by the documented Shoonya WS contract
   (`t:"a"` / `t:"tk"` / `t:"tf"` / `t:"om"`). The 317 `[ProtoContract]` / 5,485 `[ProtoMember]` figures
   remain useful only if the vendor later grants a supported CozyMQ integration.
2. **Order-update head-of-line blocking.** One WS socket carries quotes *and* order updates (Â§7.6).
   Mitigation is designed in, not retrofitted: `t`-field fast path before full deserialize.
3. **~~Broker integration is contractual~~ â€” resolved by decision.** Use the official API. See Â§12.
4. **Local MongoDB is on 4.4.2 (EOL).** Driver compatibility and a server-upgrade path must be
   planned; the app must degrade gracefully when Mongo is absent (developer laptops, CI).
5. **Observability first.** Instrument the existing app (`.NET` counters / ETW) for a baseline on
   Â§6.1â€“6.6; instrument the new core from day one (`tracing` spans + counters).

### 7.4 Suggested delivery phases

| Phase | Scope | Exit criteria |
|---|---|---|
| 0 | `fv-core` + `fv-feed` protocol/merge/replay; no network needed | replay fixture produces merged quotes; tests green |
| 1 | Live WS ingest + Mongo master read + coalescing engine; headless `fv-cli` | 500 scrips, p99 tickâ†’delta < 50 ms, < 150 MB RSS |
| 2 | P0 UI: login, market watch, snap quote, order entry, order/trade/position books, limits | side-by-side with old terminal |
| 3 | P1: depth, option chain, alerts, baskets, slicing, CO/BO, workspaces, settings, toasts | daily-driver parity for one trader |
| 4 | P2: charts + greeks/payoff, top-N, mutual funds, equity SIP, Excel bridge (Windows-only, optional) | feature-parity sign-off |
| 5 | P3: dealer/RMS/BLU/entity admin, plugin SDK | only if dealer desks are in scope |

### 7.5 Non-goals unless explicitly requested

Dealer/RMS back-office (entity manager, role manager, broker-level funds, BOD allocation, risk
baskets, BLU line/IOI trading, IVR) is a *separate product surface* already served by the vendor's
web back-office. Migrating it is a multi-quarter effort on its own.

### 7.6 Transport trade-off â€” CozyMQ bus vs official WebSocket

**[Observed]** legacy transport Â· **[Documented]** Shoonya API Â· **[Estimated]** cost figures.

| Dimension | CozyMQ (observed) | Shoonya WS (documented) | Impact |
|---|---|---|---|
| Framing | raw TCP, binary | TLS + WS frames | **+~50 B/msg** (~30 % on a 150 B tick) |
| Payload | protobuf-net | JSON, 2-char keys, **diff-only after snapshot** | 3â€“8Ã— more bytes |
| Order channel | **dedicated socket :5801** | shared with the tick feed | **worse p99** |
| Connections | 6 independent sockets | **1 per session (hard rule)** | simpler, but coupled |
| Reconnect | 6-way re-subscribe storm | 1 reconnect + re-subscribe | **better** |
| Encryption | `tcp://` â€” plaintext in the observed logs | `wss://` â€” TLS | **better** |
| Order throughput cap | not published | **~10 req/sec** | **dominates all other terms** |

**Verdict: yes, the wire is measurably slower â€” and it does not matter, except for order-ack tail
latency, which is designed around.**

The `~10 req/sec` order limit is the binding constraint. At that ceiling, a 2 ms vs 0.2 ms parse
difference is noise next to the broker OMS â†’ exchange â†’ ack round trip (tens of ms).

**Why JSON is not a problem here.** The feed is diff-streamed, not full-payload:

| Frame | Meaning | Approx size |
|---|---|---|
| `t:"tk"` | full snapshot, once per subscribe | ~300â€“400 B |
| `t:"tf"` | **only changed fields** | ~60â€“150 B |
| `t:"om"` | order lifecycle event | ~250â€“400 B |

Measured on this machine with the implementation in `crates/fv-feed` (`fv-cli bench`, release build):

| Stage | Rate | Per frame |
|---|---|---|
| classify (`t` fast path) | 105,235,464 ops/s | **0.01 Âµs** |
| parse only | 266,451 ops/s | 3.75 Âµs |
| parse + merge into `QuoteBook` | 225,849 ops/s | 4.43 Âµs (46.8 MB/s) |

At 5,000 frames/s the whole market-data path costs **4.43 Âµs Ã— 5,000 â‰ˆ 22 ms of CPU per second,
about 2 % of one core**. The order-frame fast path is effectively free, which is precisely what
protects ack latency. (The earlier estimate of <0.5 % was optimistic; the measured figure is still two
orders of magnitude below what the legacy dispatcher + logging path consumed.)

**The three risks, with the mitigation that is now implemented in `crates/fv-feed`:**

1. **Head-of-line blocking (the real one).** Order updates share the socket with ticks.
   â†’ *Fast-path routing*: read `t` (always the first key) and dispatch `om` to a dedicated channel
   **before** any market-data deserialization. A tick backlog can then never delay order state.
2. **Parsing in the webview.** Never `new WebSocket()` + `JSON.parse` in the UI. â†’ All ingest lives in
   the Rust core; the webview only ever receives coalesced 30â€“60 Hz delta frames over Tauri IPC.
3. **Subscription/rate discipline.** REST quotes are `~1 req/sec per instrument`; never poll. â†’
   All continuous data comes from the socket; Mongo answers historical/lookup questions (Â§7.9).

Net effect on the metrics that matter:

| Metric | Old terminal | New (WS + Rust) | Direction |
|---|---|---|---|
| Cold start | seconds (90.8 MB JSON parse) | <1 s (Mongo + in-process index) | **much better** |
| Tick â†’ pixel | ~16â€“300 ms (dispatcher + grid) | 16â€“33 ms (coalesced frame) | **much better** |
| Order submit â†’ ack | broker-dominated | broker-dominated | unchanged |
| CPU at 500 scrips | high, GC churn | low, no GC | **much better** |
| p99 order ack under tick burst | protected (separate socket) | **at risk** â†’ mitigated by fast path | designed |

### 7.7 Measurement plan (so the trade-off is falsifiable)

**Status: M1/M2/M5/M7 are implemented as `fv-cli` and run without credentials or market hours**
(`fv-cli bench`, `fv-cli replay`, `fv-cli doctor`). M3/M4/M6 need a live session.

| # | Test | Instrumentation | Pass criterion | Status |
|---|---|---|---|---|
| M1 | **Tick throughput** â€” 200 scrips, peak hours, 30 min | frames/s, bytes/s, parse Âµs p50/p99 | â‰¥ 3,000 frames/s sustained with <1 % dropped | harness âœ… / live â¬œ |
| M2 | **Tick â†’ delta frame** latency | monotonic timestamp at socket read vs emit | p99 < 50 ms | `Coalescer` tests âœ… / live â¬œ |
| M3 | **Order-ack latency under burst** | synthetic 5k-frame burst immediately before a test order | p99 ack â‰¤ baseline + 20 ms | â¬œ (needs live) |
| M4 | **Core CPU/RSS** | `tracing` counters + OS metrics | <10 % of one core, <150 MB RSS at 500 scrips | â¬œ (needs live) |
| M5 | **Cold start** | process start â†’ first rendered quote | <1 s | master load âœ… 2.7 s / render â¬œ |
| M6 | **Reconnect cost** | drop socket, measure data-restored time | <2 s to full quote set | â¬œ (needs live) |
| M7 | **JSON parse budget** | swap `serde_json` â†’ `simd-json`, compare | if gain <10 %, keep `serde_json` | baseline âœ… (4.43 Âµs/frame) |

**Measured baseline today (release, x86-64 Windows, local Mongo 4.4.2):**

| Metric | Value |
|---|---|
| classify only | 105,235,464 ops/s (0.01 Âµs/frame) |
| parse only | 266,451 ops/s (3.75 Âµs/frame) |
| parse + merge | 225,849 ops/s (4.43 Âµs/frame, 46.8 MB/s) |
| instrument master load | 182,901 docs in **2.69 s** |
| instrument search (`RELIANCE`) | 5 ranked hits, exact â†’ prefix |
| option chain (`NIFTY`, one expiry) | 550 legs, strike-ordered |

M7 verdict on this baseline: parsing is **2 % of one core at 5,000 frames/s**, so `simd-json` would
optimise something that is not the bottleneck. Do not add it before M1 shows a real ceiling.

### 7.8 Cross-platform design (Windows / macOS / Linux)

The product must run on all three. Everything below is a design constraint, not an afterthought.

**Rule: the core is platform-neutral.** `fv-core`, `fv-feed`, `fv-master`, `fv-store`, `fv-engine`,
`fv-market`, `fv-analytics` contain **zero** `cfg(windows)` / OS-specific code. Only the shell and
genuinely OS-bound integrations are conditional.

| Concern | Windows | macOS | Linux | Design decision |
|---|---|---|---|---|
| UI | WebView2 | WKWebView | WebKitGTK | Tauri picks per-OS; **no OS-native UI widgets** |
| Excel link (XLL/RTD) | âœ… | âŒ | âŒ | `#[cfg(windows)]` optional crate; excluded from default build |
| Secret storage | Credential Manager | Keychain | Secret Service | `keyring` crate abstracts all three |
| App data path | `%APPDATA%` | `~/Library/Application Support` | `$XDG_DATA_HOME` | `directories` crate; never hardcode |
| Fonts | Segoe UI | SF | varies (may lack a UI font) | **ship fonts**; no OS font assumptions |
| Tray / global hotkeys | Win32 | Cocoa | X11/Wayland | Tauri plugins; document Wayland limitations |
| Single instance | mutex | NSDistinct | lockfile | `tauri-plugin-single-instance` |
| Updater | Squirrel â†’ Tauri | `.app` bundle | AppImage/deb | `tauri-plugin-updater`; separate signing keys |
| Numeric formatting | ICU | ICU | ICU | fixed-format numbers in core; locale only for display |
| Time zones | â€” | â€” | â€” | **store UTC, render IST explicitly**; never rely on host TZ |
| Endianness | â€” | â€” | â€” | any on-disk binary format is **explicit little-endian** |
| Crypto/TLS | SChannel | Security.framework | OpenSSL/rustls | **`rustls` everywhere** for identical behaviour |
| MySQL/Mongo drivers | â€” | â€” | â€” | `mongodb` crate is pure Rust + rustls â†’ same on all OS |
| File watching / locks | â€” | â€” | â€” | `notify` crate; avoid OS-specific locking |

**Determinism requirements** (they prevent "works on my machine"):

- Money is `i64` **paise**, never `f64`. Prices cross the wire as strings (`"1287.30"`); convert once at
  the boundary. This also kills the old paisaâ†”rupee float-conversion bugs.
- Quantities are `i64`; lot sizes `i64`.
- All timestamps `chrono::DateTime<Utc>`; display layer formats IST.
- Sorting/ranking uses explicit comparators (no locale-collation dependence).
- No `usize` in persistent formats (32/64-bit differences).

**CI matrix:** `windows-latest`, `macos-latest`, `ubuntu-latest`, `cargo test --workspace`, plus
`cargo clippy -D warnings` and `cargo fmt --check`. Mongo-dependent tests are gated
(`FV_MONGO_URI`), so the suite passes without a database.

### 7.9 Local MongoDB as the cache of record

**[Observed]** on this machine (`mongodb://127.0.0.1:27017`, server **4.4.2**). The local instance
already holds the reference data that the old terminal re-parsed from a 90.8 MB file â€” so the new
core must **read from Mongo first and never re-query the broker for data it already has**.

#### Observed databases & collections

| DB | Collection | Docs | Role in the new app |
|---|---|---:|---|
| `STOCK` | `FINVASIA_SYMBOL` | 182,901 | **Primary symbol master** â€” Finvasia-native `EXCHANGE` + `token` (string) + `tradingSymbol`, `expiry`, `strikeType`, `strikePrice`, `underlying`, `lotSize` |
| `STOCK` | `INSTRUMENT` | 131,903 | Kite-shaped master (`instrument_token`, `tradingsymbol`, `instrument_type`, `segment`, `strike`, `lot_size`, `expiry`) â€” cross-reference / back-compat |
| `STOCK` | `INSTRUMENT_DETAIL_TYPE` | 557,610 | Segment/type detail (Java `InstrumentDetailType`) |
| `STOCK` | `SCRIP` | 2,923 | **Curated tradable universe** (`symbol`, `sector`, `hasDerivative`, `fragmentGroup`, `active`) |
| `STOCK` | `SNAPSHOT_INTRADAY` | 321,240 | Intraday OHLC/OI snapshots â€” historical answer source |
| `STOCK` | `DATA_STORED_TRACKER` | 87,061 | Backfill ledger (`underlying`, `instrument`, `dateUpdatedOn`, `foundInstrument`, `notFoundCount`) |
| `STOCK` | `EXPIRY_LOT_DETAILS` | 14 | `underlying` + `expiry` + `type` â†’ `lotSize` |
| `STOCK` | `HOLIDAY_LIST` | 123 | Trading holidays |
| `STOCK` | `ALERTS_MANAGEMENT` | 1,341 | Alert definitions |
| `STOCK` | `FV_ORDERS` | 0 | Target collection for the new app's order journal (currently unused) |
| `STOCK` | `CONFIGS_GLOBAL` | 30 | Global config |
| `STOCK` | `COMMAND_QUEUE` | 0 | Cross-process command channel (potential IPC to the app) |

Field-shape notes (important for correctness â€” the two masters differ in casing):

```
FINVASIA_SYMBOL  { Symbol, underlying, exchange, token:"158614", lotSize, tradingSymbol,
                   expiry:"2027-02-23", strikeType:"PE", strikePrice:1680 }
INSTRUMENT       { instrument_token:282257669, exchange_token:1102569, tradingsymbol:"BANKEX26NOVFUT",
                   name, last_price, tick_size, instrument_type:"FUT", segment:"BFO-FUT",
                   exchange:"BFO", strike, lot_size, expiry:ISODate, __v }
```

`exchange` + `token` is the join key to the WebSocket (`{"t":"t","k":"NFO|158614"}`).

#### Measured load (new core, release build)

`fv-cli master stats` against the live local database:

```
loaded 182,901 instruments in 2.69 s
underlyings     19,442
NSE 10,103 Â· BSE 12,950 Â· NFO 81,110 Â· BFO 47,597 Â· MCX 18,014 Â· CDS 13,124
NIFTY expiries  18 (2026-12-01 â€¦ 2031-06-24)
option chain    NIFTY 2026-12-01 â†’ 550 legs, strike-ordered
```

For comparison, the legacy terminal parses a **90.8 MB JSON file** on cold start before it can show a
single quote. 2.7 s is already a large improvement but is not the <1 s target: the remaining cost is
Mongo cursor iteration plus BSONâ†’struct mapping. Next step is `RawDocumentBuf` zero-copy reads, or a
cached binary snapshot refreshed from Mongo.

#### Index finding â€” dead indexes (worth fixing, it is free throughput)

**[Observed]** `INSTRUMENT` declares indexes on `tradingSymbol`, `dateStoredFor`, `candle`,
`kiteInstrumentToken`, `strikeType`, `strikePrice` â€” but its documents use `tradingsymbol`
(lowercase) and contain **none** of the other fields. Those indexes are idle storage and slow down
every insert/update. The same generic index set is duplicated across `FINVASIA_SYMBOL`,
`EXPIRY_LOT_DETAILS` and `DATA_STORED_TRACKER`.

Recommended before go-live (measured, reversible):

```js
// verify usage first
db.INSTRUMENT.aggregate([{ $indexStats: {} }])
// drop the ones with zero accesses, then create what the app actually queries
db.INSTRUMENT.createIndex({ exchange: 1, instrument_token: 1 }, { unique: true })
db.INSTRUMENT.createIndex({ exchange: 1, tradingsymbol: 1 })
db.FINVASIA_SYMBOL.createIndex({ exchange: 1, token: 1 }, { unique: true })
db.FINVASIA_SYMBOL.createIndex({ underlying: 1, expiry: 1, strikeType: 1, strikePrice: 1 })
```

#### Cache policy per data class

| Data class | Source of truth | Freshness rule | On miss / stale |
|---|---|---|---|
| Symbol master | Mongo (`FINVASIA_SYMBOL`) | per-exchange `UpdTime` stamp; refresh daily pre-market or on demand | call `GetInstrument`-style endpoint once, upsert, never per scrip |
| Intraday snapshots | Mongo (`SNAPSHOT_INTRADAY`) | append-only; `DATA_STORED_TRACKER` prevents re-fetch | fetch the missing window only |
| Live ticks / depth | **WS only â€” never cached as truth** | in-memory, coalesced | resubscribe |
| Orders / trades / positions | WS `t:"om"` for live; REST for reconciliation | reconcile on connect + every N min | REST, then journal to `FV_ORDERS` |
| Holdings / limits | REST | on login + on demand | REST |
| Holidays / lot sizes | Mongo | static | read once |

**Hard rule: no per-scrip REST calls.** The old app issued `Request sent for Get Scrips Info` per
strike (Â§6.4). The new app reads the master from Mongo; a scrip is only fetched when *genuinely absent*
and then written back.

#### Graceful degradation (required for mac/linux dev and CI)

`fv-store` exposes a `MasterStore` trait with three implementations:

1. `MongoMaster` â€” production.
2. `SqliteMaster` â€” embedded fallback so the app runs without a Mongo server.
3. `MemoryMaster` â€” tests.

Selection at startup: `FV_MONGO_URI` â†’ Mongo; else a local embedded snapshot; else a clear, actionable
error. **The app must never hard-crash because Mongo is down** â€” it degrades to "live feed only, no
history".

#### Server version risk

Local server is **4.4.2 (EOL, April 2024)**. The Rust driver (`mongodb` 3.x) works with 4.x, but 4.4
lacks newer features and receives no security fixes. Plan: test against 4.4.2 and 7.x; target 7.0 for
deployment. Keep the repository layer free of version-specific aggregation so both work.

---

## 8. Risk, security & compliance notes

| Item | Observation | Action |
|---|---|---|
| Transport | Session log shows `tcp://` (plaintext) for FEED/ORDER/REQUEST/TCHLINE on :5817/:5801/:5809/:5813. CozyMQ *supports* `ssl://` and exposes `ValidateServerCertificate` | New client uses `wss://` only; no plaintext order path (DR-1) |
| Credentials | Login uses user id + PIN/password + OTP/TOTP (`OTP_TOTP=True`); the **public API is OAuth 2.0** (app key + secret â†’ auth code â†’ access token) | Store tokens/secrets in OS keychain via `keyring`, never on disk in plaintext |
| Local plaintext | Session logs contain client ids, order ids, scrip and exchange messages | Encrypt or redact logs; cap retention (see Â§6.5) |
| Shared crypto constants | `CozyMQ` embeds a static key/IV-looking literal (`D2D6A3D98346956B22CA59795E4A181C`) and a license-SHA literal | Vendor-internal; **not** used by the new codebase (DR-1) |
| Licensing | Vendor is Kambala Solutions; redistribution of their DLLs is likely prohibited | Not applicable to the rewrite â€” official API only (DR-1) |
| Exchange compliance | SEBI Algo ID framework governs automated order flow; higher order rates require an approved algo ID | Complete SEBI algo registration before enabling automated/strategy order placement |
| Mongo server | Local instance is 4.4.2 (EOL); no security patches | Plan upgrade to 7.x; repository layer kept version-agnostic (Â§7.9) |
| Data retention | Session logs kept indefinitely (files from 2025-07 through 2026-11 present) | Define retention under your compliance policy |

---

## 9. Feature-parity checklist for the rewrite

Copy into your tracker; each row is one shippable slice.

```
[ ] P0  Login (user/PIN/OTP-TOTP), logout, session recovery
[ ] P0  Instrument master load + fast search (symbol/ISIN/company/near-symbol)
[ ] P0  Market watch: groups, add/remove/reorder, column chooser, save/restore
[ ] P0  Live ticks (LTP/chg/vol/OI), trend colours, sortable grid
[ ] P0  Snap quote with custom fields + compact mode
[ ] P0  Order entry: LMT/MKT/SL-L/SL-M, CNC/MIS/NRML/MTF, disclosed qty, remarks
[ ] P0  Order book (open/closed split) with all 29 fields
[ ] P0  Trade book, positions book, square-off
[ ] P0  Holdings + limits + margin used/available
[ ] P0  Settings: themes, fonts, title bar, order defaults, colours, notify
[ ] P0  Auto-update
[ ] P1  Depth 5/20, floating scrip view, spot rates, index watch/title bar
[ ] P1  Option chain with greeks + OI, der chain, implied vol
[ ] P1  Top-N movers, market/exchange status, exchange messages, UI messages
[ ] P1  AMO, CO/BO with trailing SL, trigger/GTT, basket, slicing, multi-leg
[ ] P1  Order history, product conversion + report, order/basket/span margin
[ ] P1  Alerts (day + GTT), position alerts, toasts/sound, notification centre
[ ] P1  Workspaces, shortcut manager, CSV export/import, diagnostics
[ ] P2  Desktop + web charts with indicator set, VWAP, pivots, scanners
[ ] P2  Payoff/greeks analytics, option & yield calculators
[ ] P2  Mutual funds, equity SIP
[ ] P2  Excel bridge (named pipe + optional XLL/RTD)
[ ] P3  Dealer/RMS/BLU/entity admin, plugin SDK, IVR, OFS/CNT/block
```

---

## 10. Open unknowns

| # | Unknown | How to resolve |
|---|---|---|
| U1 | Exact CozyMQ frame layout (length prefix, control-byte positions, LZ4 flag semantics) | pcap of one login/subscribe + differential replay |
| U2 | All 5,485 protobuf field numbers | metadata reader over `ProtoMemberAttribute` blobs; validate vs live frames |
| U3 | Whether production uses TLS on 5801/5809/5817/5813 | broker confirmation + pcap |
| U4 | Reconnect backoff / jitter parameters | runtime observation (strings show the path, not the constants) |
| U5 | Server-side subscription limits per role (30 observed for investor) | broker docs / role matrix |
| U6 | Which features are server-gated by `UserPrivilege` (`Investor`, `Dealer`, `Admin`) | login as each role |
| U7 | Actual CPU/GC profile of the current build | ETW/PerfView trace on a live session |

---

## 11. Appendix â€” key symbol index

| Symbol | File | Meaning |
|---|---|---|
| `CozyMQFramework.cozyMain` | `CozyMQ.dll` | bus client: `AddDevice`, `AddSubscription`, `SendRequest`, `PollSockets` |
| `CozyMQFramework.CozyDevice` | `CozyMQ.dll` | one socket: `ConnectDevice`, `Read`, `WriteToDevice`, `Reconnect` |
| `CozyMQFramework.CozyDeviceCallback` | `CozyMQ.dll` | callback maps: `AddSubSCBmap`, `AddReqSCBmap`, `Process` |
| `CozyApi.NorenConnection` | `CozyMQ.dll` | `CheckLicense`, `GetProfile`, `GetAddressMessage` |
| `CozyMQFramework.CozyConstants` | `CozyMQ.dll` | all protocol constants (`SUB`, `R SUB`, `fwd_all`, `LZ4`, â€¦) |
| `NorenApi.NorenApiMain` | `NorenApi.dll` | `CreateConnection`, `ConnectToBrokers`, `AddSubscription`, `SetHandOverRequest` |
| `NorenApi.NorenDeviceCb` | `NorenApi.dll` | reconnect + `ReconSubscriptions` / `ReconRegistration` |
| `NorenApi.FeedBroker` | `NorenApi.dll` | `SubscribeFor_MW_Update`, `SubscribeFor_IV_Updates`, auction/lagged feeds |
| `NorenApi.TouchLineBroker` | `NorenApi.dll` | `SubscribeFor_DEPTH_Update` |
| `NorenApi.OrderBroker` | `NorenApi.dll` | `PlaceNewOrder`, `CancelOrder`, `SendReqToModifyOrder`, `SendAutoSquareOff` |
| `NorenApi.ReqBroker` | `NorenApi.dll` | all request/response RPCs (login, limits, positions, masters, admin) |
| `NorenApi.UFT_Convertor` | `NorenApi.dll` | `ProtoSerialize` / `ProtoDeSerializer`, price & time conversion |
| `NorenApi.PriceConvertor` | `NorenApi.dll` | paisaâ†”rupee, price formatting by precision |
| `NorenPlugin.ApiUtils` | `NorenPlugin.dll` | `ProtoSerialize`, `ProtoDeSerializer` |
| `UFTrader.UFT_Stores.MarketWatchStore` | `Finvasia.exe` | group + scrip watch state, save/restore, subscription updates |
| `UFTrader.UFT_Stores.MTObservableCollection<T>` | `Finvasia.exe` | thread-safe observable collection used by every grid |
| `UFTrader.UFT_Stores.SequencingService` | `Finvasia.exe` | orders collection-change delivery |
| `UFTrader.ScripsDb` | `Finvasia.exe` | in-memory instrument DB + all search/option-chain queries |
| `UFTrader.SqlLiteDb` | `Finvasia.exe` | thin SQLite wrapper |
| `UFTrader.UFTConfig` / `SettingsStore` | `Finvasia.exe` | config + per-grid preference persistence |
| `UFTrader.PerfMonitor` | `Finvasia.exe` | `Start`/`Stop` timing probes |
| `Finvasia.exe` app settings | `Finvasia.exe.config` | endpoints, key, chart endpoints, CO/BO trail, 2FA label |

---

## 12. Decision record

**DR-1 â€” Transport: official Shoonya API, not the CozyMQ bus.** *(Accepted)*

- **Context.** The shipped terminal speaks a proprietary protobuf/CozyMQ bus (6 sockets, device
  identity + licence handshake). Re-implementing it requires bypassing an access control and
  reimplementing undocumented framing, plus a vendor-licence question.
- **Decision.** Build on the documented Shoonya REST + WebSocket API (`api.shoonya.com`).
- **Consequences.** Wire is somewhat slower (Â§7.6) and order-ack isolation must be engineered
  (Â§7.6 risk 1). In exchange: no schema recovery, TLS by default, a supported contract, and
  ~6 months of Phase-0 work removed.
- **Reversal cost.** Low for the UI/engine; the `fv-feed` transport is behind a trait, so a vendor
  CozyMQ integration could be added later without touching the core.

**DR-2 â€” Local MongoDB is the cache of record for non-live data.** *(Accepted)*

- Already-populated locally (`INSTRUMENT` 131,903 / `FINVASIA_SYMBOL` 182,901 / `SNAPSHOT_INTRADAY`
  321,240). Eliminates the 90.8 MB startup parse and per-scrip server queries.
- Live feed is **never** sourced from Mongo; Mongo supplies history, masters and reference data only.
- Requires degrading gracefully when Mongo is unavailable (Â§7.9).

**DR-3 â€” Cross-platform from day one.** *(Accepted)*

- Windows, macOS, Linux. Platform-neutral core; OS-bound features (Excel XLL) isolated behind
  `cfg(windows)`. Deterministic money (i64 paise), UTC timestamps, explicit LE on-disk formats.

**DR-4 â€” Ingest never runs in the webview.** *(Accepted)*

- Socket + JSON parse + state live in Rust; the webview receives coalesced delta frames only.

---

## 13. Implementation status

Phase 0/1 is implemented and verified ([`README.md`](README.md), [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)).

| Crate | Scope | Tests |
|---|---|---|
| `fv-core` | `Price` (i64, 6 dp), `SymbolKey`, `Quote` | 17 |
| `fv-feed` | frame decode, `QuoteBook` diff-merge, subscription batching, replay, legacy WS transport + heartbeat + scope fallback + live subscribe commands | 42 |
| `fv-master` | `MemoryMaster` index/search/option-chain, Mongo loader (both collection shapes), multi-term search | 18 |
| `fv-store` | memory + Mongo backends, tracker freshness rule, snapshot upsert, order journal | 16 |
| `fv-engine` | `Coalescer` (bounded, deterministic), `TokenBucket` (venue limits) | 12 |
| `fv-bridge` | `MarketData` state, watch/chain diffing, subscription budget, DTO formatting | 21 |
| `fv-cli` | build-date calendar + CLI contract as a real process | 8 |
| `ui/` | formatting + `QuoteTable` merge/prune/placeholder (vitest) | 14 |
| **Total** | | **148** |

Gates: `cargo test --workspace` (134 Rust), `npm test` (14 UI), `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check`, `svelte-check` (0 errors/warnings).
Shipped via `scripts/release.ps1` into `releases/` as a desktop app plus a CLI.

### Phase 2a â€” desktop UI (implemented)

`fv-bridge` + `src-tauri` + `ui/` add a Tauri 2 window over the same engine:

| Piece | Behaviour |
|---|---|
| Market watch | coalesced deltas, trend colouring, placeholder row immediately on add |
| Snap quote | merged quote + instrument metadata, read-only (order entry is Phase 2b) |
| Instruments | debounced master search (Mongo, else bundled sample) and watch add/remove |
| Diagnostics | backend, frame rate, decode errors, demo/live feed controls |
| Status bar | source, backend, instrument count, watch budget, frames/s, build |

Threading: one producer â†’ one consumer that owns the state lock â†’ 60 Hz delta events. The lock is
never held across an `.await`, `stop_feed` aborts the producer, and the frontend releases every
listener on teardown.

Two defects were caught by the new tests:

1. **`c` was never mapped to `prev_close`**, so change/trend would have rendered blank for every
   instrument. The venue's `c` is the reference close; it now populates both.
2. **A watched instrument had no row until its first tick**, so adding an illiquid scrip looked like
   a no-op. `QuoteTable` now inserts a placeholder row immediately.

One defect was caught by measuring the running app rather than by a test:

3. **Replay pacing was capped at ~64 Hz on Windows.** A per-frame `sleep(1/hz)` interacts badly with
   the ~15.6 ms timer granularity: a 120 Hz demo measured 65/s. Pacing now measures real elapsed time
   and replenishes a frame budget, which holds 120/s (verified 119â€“122/s) and will replay a
   high-rate real capture at its true speed.

End-to-end verification of the shipped app (`FV_AUTOSTART_DEMO=1`, debug log): window opens,
182,901 instruments load from Mongo, 3 capture symbols enter the watch, frame counter climbs
~120/s with 0 decode errors.

A fourth defect was reported from actually launching the released build:

4. **The desktop window showed `ERR_CONNECTION_REFUSED` for `localhost:5173`.** Cause:
   `tauri-build` marks a build as *dev* unless the `tauri` crate is compiled with `custom-protocol`;
   a plain `cargo build --release` therefore loads `devUrl` instead of the embedded frontend. The
   process, backend and master load all worked, which is why the earlier smoke test passed â€” it
   verified the Rust side but not that the webview rendered. Fixed by building through
   `npm run tauri:build`, and `scripts/release.ps1` now probes port 5173 during release and fails if
   the shipped app reaches for the dev server.

### Phase 2b â€” live data (implemented)

Reported symptom: *"start demo feed doesn't do anything"*. Two independent causes, both found by
attaching to the running WebView2 over the DevTools protocol rather than guessing:

5. **No Tauri capability file**, so `plugin:event|listen` was denied by the ACL
   (`Command plugin:event|listen not allowed by ACL`). Every `listen()` failed, the init routine
   aborted before hydrating, and the grid stayed empty. Added
   `src-tauri/capabilities/default.json` with `core:default`, and made listener registration
   per-listener so one failure can no longer blank the app.

6. **The live protocol was the wrong generation.** The documented OAuth flow
   (`wss://â€¦/NorenWSAPI/`, `t:"a"` + `accesstoken`) does not accept the sessions this account holds.
   The working protocol â€” confirmed live â€” is the legacy one:
   `wss://api.shoonya.com/NorenWSTP/`, frame `{"t":"c","uid","actid","susertoken","source":"WEB"}`,
   ack `{"t":"ck","s":"OK"}`, heartbeat `{"t":"h"}` every 3 s. Web-minted tokens require
   `source:"WEB"`; the client falls back to `"API"` on `NOT_OK`.
   Also: `NorenWClientTP` (used by the older in-house client) now returns **502**; the live REST base
   is `NorenWClientAPI`.

7. **`report_schoonya` in `fv-cred.json` is a password, not an account id.** Using it as `actid`
   made the handshake fail silently. `actid` now defaults to the user id.

8. **The venue serves one websocket per token.** A pre-flight probe connection left the real
   connection silent. The probe was removed; connection state is reported from the single session via
   `FeedState`, so the UI shows *connected* only after the handshake actually succeeds.

Verified end-to-end through the UI: session adopted from `CONFIGS_GLOBAL` (issued 2026-11-26),
`live feed connected source=WEB`, NTPC 321.80 â†’ 321.70 with bid/ask and volume, ACC 1262.80,
182,901 instruments loaded.

Notable defects found and fixed during implementation â€” each is a class of bug the legacy code had:

1. **Malformed JSON silently became an "unknown frame".** Now validated with `IgnoredAny`, so
   corruption is an error while a genuinely new frame type stays observable.
2. **Mongo driver panicked at runtime teardown** (background pool outliving tokio). Fixed with an
   explicit `Store::shutdown()` / `Client::shutdown()` on every path. This is the resource-hygiene
   rule from Â§6.5 applied to the new stack.
3. **Backoff cap ignored jitter** (60 s Ã— 1.2 = 72 s). Base now capped at 50 s with a hard 60 s clamp,
   and the jitter band is asserted by a test.
4. **`Quote` derived `Default`**, which would have allowed a quote with an empty `SymbolKey`. Removed;
   construction goes through `Quote::empty(key)` so the invariant cannot be violated.

Deliberately still absent: `fv-bridge`, `src-tauri`, `ui/`, `fv-market` (REST). Those are Phase 2 and
need the P0 UI decisions in Â§5.1â€“5.4 plus OAuth app registration.

---

### Phase 2c - relaxed search + live subscription (implemented)

Two usability gaps reported after using the app:

9. **Search was a single substring match**, so `"nifty 25800"` found nothing. It is now multi-term
   with AND semantics: every term must match a symbol prefix, underlying prefix, symbol/name
   substring, or a strike price (strike compared as integer strings - no float arithmetic).
   `"nifty 25800"` and `"25800 nifty"` return the same contracts. Verified against the live master:
   6 NIFTY 25800 CE/PE legs across expiries; `"nifty 99999"` correctly returns nothing.

10. **Adding an instrument did not subscribe it.** The watch change only updated the UI; the socket
    subscribed once at connect. `fv-feed` now exposes `FeedCommand::{Subscribe, Unsubscribe,
    Shutdown}` on a channel the session selects on, so a watch change is applied to the open socket
    and the full set is re-sent after any reconnect. Verified live: adding
    `NIFTY01DEC26C25800` logged `subscribed while live count=1` and the row began ticking
    (245.65 -> 245.30 with bid/ask and volume) on the *same* connection - no reconnect.

---

### Phase 2d - option chain, with a hard subscription budget (implemented)

The legacy screenshot shows ~10 windows. Only the option chain is implemented so far; the trading
books need the REST scope decision (see below).

11. **Option chain**: full expiry browsable from the local master (550 NIFTY legs), but only a
    **window around the money** is subscribed (default ±10 strikes = 42 legs). The UI shows
    `N legs / M live` so the difference is obvious, and nothing is subscribed until the user presses
    *Load and subscribe*.

12. **A 550-leg chain polluted the market watch.** `QuoteTable.upsert` appended unknown delta keys,
    so every chain leg appeared in the watch grid and was labelled "watching". Deltas now update only
    rows that are already on the watch list.

13. **Hard ceiling of 100 simultaneous subscriptions** across watch + chain
    (`MAX_TOTAL_SUBSCRIPTIONS`). Growing the watch list trims the chain group; overflow is returned in
    the plan and surfaced in the UI. `status.subscribed` / `subscription_limit` expose the budget, and
    the status bar renders `subscribed N/100`.

Verified with one connection: 550-leg chain browsable, **42 subscribed**, `subscribed 45/100`
(3 watch + 42 chain), bid/qty/LTP/ask/qty/change/%/volume all populated, and every chain row showing
*Add* rather than "watching".

---

### Provenance of this document

| Evidence | Source |
|---|---|
| Assembly identities, versions, type/method/field counts | REA managed-artifact + managed-members inspection of the 11 assemblies listed in Â§1.1 |
| All type/method/field/string names quoted | CLI metadata of `Finvasia.exe`, `NorenPlugin.dll`, `NorenApi.dll`, `CozyMQ.dll`, `Plugins/*.dll` |
| `[ProtoContract]` = 317, `[ProtoMember]` = 5,485 | full scan of `NorenPlugin.dll` custom-attribute table |
| Endpoints and ports | `Finvasia.exe.config`, `NorenTrader.exe.config`, `SessionLogs/*.txt` (`COZY` lines) |
| Exchange-message flood ratios | line censuses of `SessionLog20251014T215103.txt` and `SessionLog20251023T221455.txt` |
| Scrip counts, master size, subscription cap | `UserSettings.json`, `ScripsData.json` (90.8 MB), `SessionLog*.txt` |
| Feature list | `doc/uft_document.chm` (20 chapters, decompiled) **plus** window/VM/API symbols in the binaries |
| MongoDB collections, doc counts, sample shapes, indexes | `mongo` shell queries against `127.0.0.1:27017` (Â§7.9) |
| WS protocol, rate limits, 1-connection rule | shoonya.com official API documentation |

*No target code was executed. All runtime numbers are taken from files the application wrote.*
