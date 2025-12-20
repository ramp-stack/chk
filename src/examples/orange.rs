use chk::*;

pub struct Orange;

impl Application for Orange { // needs to be a fixed vector of 6 with minimum 1
    fn start(ctx: &mut Context) -> Vec<Root> {
        ctx.state().set(NewTransaction::default());
        vec![Root::new(RootContent::icon("wallet"), BitcoinHome::build())]
    }

    fn theme(_ctx: &mut Assets) -> Theme { Theme::Dark(Color::from_hex("#eb343a", 255)) }

    fn on_event(ctx: &mut Context, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if event.downcast_ref::<TickEvent>().is_some() {
            if let Some(tx) = ctx.state().get_mut::<NewTransaction>() {
                if let Some(usd_value) = tx.inner.amount.usd() {
                    tx.inner.amount.btc = format!("{:.8} BTC", usd_value / 1_000_000_000.00);
                }
            }

            ctx.state().get_named::<String>("AddressTextInput").cloned().and_then(|address| {
                ctx.state().get_mut::<NewTransaction>().map(|tx| tx.inner.address = address)
            });

            ctx.state().get_named::<String>("AmountCurrencyInput").cloned().and_then(|amt| {
                ctx.state().get_mut::<NewTransaction>().map(|tx| tx.inner.amount.usd = amt)
            });

            ctx.state().get_named::<String>("FeeEnumerator").cloned().and_then(|val| {
                ctx.state().get_mut::<NewTransaction>().map(|tx| tx.inner.is_priority = val == "Priority")
            });
        }

        vec![event]
    }
}

#[derive(Debug, Clone)]
pub struct BitcoinHome; 
impl BitcoinHome {
    fn build() -> RootPage {
        RootPage::new("Wallet", 
            vec![
                Display::currency(12.56, "0.00001234 BTC"),
                Display::list(None, vec![
                    ListItem::plain("Bitcoin Received", "0.00001234 BTC", Some("$12.45"), "txid0"),
                    ListItem::plain("Sent Received", "0.00001234 BTC", Some("$12.45"), "txid1"),
                    ListItem::plain("Sent Received", "0.00001234 BTC", Some("$12.45"), "txid2"),
                    ListItem::plain("Bitcoin Received", "0.00001234 BTC", Some("$12.45"), "txid3"),
                    ListItem::plain("Sent Received", "0.00001234 BTC", Some("$12.45"), "txid4"),
                ], Some(ViewTransaction::build()), None)
            ], 
            None,
            RootBumper::new("Receive", Receive::build()),
            Some(RootBumper::new("Send", Send::build())),
        )
    }

    // fn on_event(&self, _ctx: &mut Context, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {vec![event]}
}

// pub struct BitcoinHome; 
// impl BitcoinHome {
//     fn build() -> RootPage {
//         RootPage::new("Wallet", 
//             vec![
//                 Display::currency(12.56, "0.00001234 BTC"),
//                 Display::list(None, vec![
//                     ListItem::plain("Bitcoin Received", "0.00001234 BTC", Some("$12.45"), "txid0"),
//                     ListItem::plain("Sent Received", "0.00001234 BTC", Some("$12.45"), "txid1"),
//                     ListItem::plain("Sent Received", "0.00001234 BTC", Some("$12.45"), "txid2"),
//                     ListItem::plain("Bitcoin Received", "0.00001234 BTC", Some("$12.45"), "txid3"),
//                     ListItem::plain("Sent Received", "0.00001234 BTC", Some("$12.45"), "txid4"),
//                 ], Some(ViewTransaction::build()), None)
//             ], 
//             None,
//             RootBumper::new("Receive", Receive::build()),
//             Some(RootBumper::new("Send", Send::build())),
//         )
//     }
// }

pub struct Receive;
impl Receive {
    pub fn build() -> Flow {
        let address = "staesuh8438iy92i984did48i";
        Flow::new(vec![Box::new(|_state: &mut State| PageType::display("Receive bitcoin", 
            vec![Display::qr_code(address, "Scan to receive bitcoin.")], 
            None, Bumper::custom("Share", Action::share(address)), Offset::Center
        ))])
    }
}

pub struct ViewTransaction;
impl ViewTransaction {
    pub fn build() -> Flow {
        Flow::new(vec![Box::new(|state: &mut State| {
            let tx = &state.get::<NewTransaction>().unwrap().inner;
            let dir = if tx.is_received {"Received"} else {"Sent"};
            PageType::display(&format!("{dir} bitcoin"), vec![
                Display::currency(12.56, "0.00001234 BTC"),
                Display::table("Transcation details", vec![
                    TableItem::new("Amount Sent (BTC)", &tx.amount.btc),
                    TableItem::new("Amount Sent", &tx.amount.usd),
                    TableItem::new("Transaction Fee", &tx.fee),
                    TableItem::new( "Transaction Total", &tx.total),
                ])
            ], None, Bumper::Done, Offset::Start)
        })])
    }
}

pub struct Send;
impl Send {
    pub fn build() -> Flow {
        let address = |_state: &mut State| PageType::input("Bitcoin address", Input::text("Bitcoin address", None, "AddressTextInput", |ctx: &mut Context| {
            ctx.state().get_mut::<NewTransaction>().map(|tx| tx.inner.address.is_empty()).unwrap_or_default()
        }), Bumper::default());
        // Some(vec![
            // QuickAction::custom("Paste Clipboard", "Pasted", |_ctx: &mut Context| {}),
            // QuickAction::flow("Scan QR Code", ScanQRCode::new()),
            // QuickAction::flow("Select Contact", SelectContact::new())
        // ])

        let amount = |_state: &mut State| PageType::input("Bitcoin amount", Input::currency("Enter send amount", "AmountCurrencyInput", |ctx: &mut Context| {
            ctx.state().get_mut::<NewTransaction>().map(|tx| tx.inner.amount.usd().map(|u| u <= 0.0).unwrap_or_default()).unwrap_or_default()
        }), Bumper::default());

        let speed = |_state: &mut State| PageType::input("Transaction speed", Input::enumerator(vec![
            EnumItem::new("Standard", "Arrives in ~2 hours\n$0.18 bitcoin network fee"),
            EnumItem::new("Priority", "Arrives in ~30 minutes\n$0.32 bitcoin network fee"),
        ], "FeeEnumerator"), Bumper::default());

        let review = |state: &mut State| {
            let tx = &state.get::<NewTransaction>().unwrap().inner;
            let speed = if tx.is_priority {"Priority (~30 mins)"} else {"Standard (~2 hr)"};
            PageType::review("Confirm send", vec![
                Display::review("Confirm address", &tx.address, "Bitcoin sent to the wrong address can never be recovered."),
                Display::table("Confirm amount", vec![
                    TableItem::new("Amount Sent (BTC)", &tx.amount.btc),
                    TableItem::new("Amount Sent", &tx.amount.usd),
                    TableItem::new("Transaction Speed", speed),
                    TableItem::new("Transaction Fee", &tx.fee),
                    TableItem::new( "Transaction Total", &tx.total),
                ])
            ])
        };

        let success = |_state: &mut State| PageType::success("Bitcoin sent", "bitcoin", "You sent $10.00");

        let on_submit = |ctx: &mut Context| println!("Broadcasting transaction... {:?}", ctx.state().get::<NewTransaction>());
        Flow::form(vec![Box::new(address), Box::new(amount), Box::new(speed)], Some(Box::new(review)), Box::new(success), on_submit)
    }
}


#[derive(Clone, Debug, Default)]    
pub struct BitcoinAmount {
    pub btc: String,
    pub usd: String,
}

impl BitcoinAmount {
    pub fn usd(&self) -> Option<f32> {
        self.usd.trim_start_matches('$').replace(',', "").parse::<f32>().ok()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Transaction {
    pub address: String,
    pub amount: BitcoinAmount,
    pub is_priority: bool,
    pub fee: String,
    pub total: String,
    pub is_received: bool,
}

#[derive(Clone, Debug, Default)]
pub struct NewTransaction {
    pub inner: Transaction
}
