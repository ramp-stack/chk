#![allow(clippy::new_ret_no_self)]
use ramp::prism;

use std::collections::HashSet;

mod state;
pub use state::*;

use chk::{
    RootInfo, FormItem, NumberVariant, Flow, Bumper, ChkTheme, AvatarIconStyle,
    Display, Offset, Context, Screen, PageType, PageBuilder, Icons, AvatarContent,
    Color, Theme, Form, Root, State, Review, Success, Message, Profile, FormSubmit,
};

use chk::items::{ListItem, Action, TableItem};

chk::run! { |_ctx: &mut Context| Orange }

pub struct Orange;
impl chk::App for Orange {
    fn roots(&self, ctx: &mut Context, theme: &Theme) -> Vec<RootInfo> {
        vec![
            RootInfo::icon(ctx, theme, Icons::Wallet, "Wallet", BitcoinHome::new(theme)),
            // RootInfo::icon(ctx, theme, Icons::Messages, "Messages", MessagesHome::new(theme)),
            // RootInfo::avatar(ctx, theme, AvatarContent::icon(Icons::Profile, AvatarIconStyle::Secondary), "Profile", MessagesHome::new(theme))
        ]
    }

    fn theme(&self) -> ChkTheme {ChkTheme::Dark(Color::from_hex("#eb343a", 255))}
}

#[derive(Debug, Clone)]
pub struct BitcoinHome;

impl BitcoinHome {
    fn new(theme: &Theme) -> PageType {
        let items = Transaction::test().iter().map(|t| {
            let _task = t.clone();
            let title = if t.is_received {"Received bitcoin"} else {"Sent bitcoin"};
            let view = vec![Screen::new_builder(theme, ViewTransaction::new(t.clone()))];
            ListItem::plain(title, &t.date, Some(&t.amount.usd), Some(Flow::new(view)))
        }).collect();

        let send = SendFlow::new(theme);
        let receive = vec![Screen::new_builder(theme, Receive::new())];

        Root::new("Wallet",
            vec![
                Display::currency(12.50, "0.00001234 bitcoin"),
                Display::list(None, items, None),
            ], //Some(Flow::new(vec![Screen::new_builder(builder, TaskDetails)])))],
            None, ("Receive".into(), Flow::new(receive)), Some(("Send".into(), Flow::from_form(send))),
        )
    }
}

pub struct Receive;
impl Receive {
    pub fn new() -> Box<dyn PageBuilder> {
        Box::new(|_: &Theme| {
            PageType::display(
                "Receive bitcoin",
                vec![Display::qr_code(&Address::generate(), "Scan to receive bitcoin.")],
                None,
                Bumper::custom("Copy Address", Action::share(&Address::generate())),
                Offset::Center,
            )
        })
    }
}

pub struct ViewTransaction;
impl ViewTransaction {
    pub fn new(transaction: Transaction) -> Box<dyn PageBuilder> {
        Box::new(move |_: &Theme| {
            let direction = if transaction.is_received {"Received"} else {"Sent"};
            PageType::display(
                &format!("{direction} bitcoin"),
                vec![Display::currency(12.50, &transaction.amount.btc)],
                None,
                Bumper::Done,
                Offset::Start,
            )
        })
    }
}

pub struct SendFlow;
impl SendFlow {
    pub fn new(theme: &Theme) -> Form {
        let closure = Box::new(move |_ctx: &mut Context, objects: &Vec<State>| {println!("Transaction {:?}", objects)}) as Box<dyn FormSubmit>;

        let review = |objects: &Vec<State>| {
            let State::Text(address) = objects[0].clone() else { todo!() };
            let btc = "0.00001234";
            let State::Number(usd) = &objects[1] else { todo!() };
            let State::Enumerator(priority) = &objects[2] else { todo!() };
            let fee = "$0.38";
            let total = "$12.30";

            vec![
                Display::review("Confirm address", &address, "Bitcoin sent to the wrong address can never be recovered."),
                Display::table("Confirm amount", vec![
                    TableItem::new("Amount Sent (BTC)", btc),
                    TableItem::new("Amount Sent", usd),
                    TableItem::new("Transaction Speed", match priority { // probably should use an enum here
                        0 => "Standard (~2 hours)",
                        _ => "Priority (~30 minutes)"
                    }),
                    TableItem::new("Transaction Fee", fee),
                    TableItem::new("Transaction Total", total),
                ])
            ]
        };

        let success = |objects: Vec<State>| {
            let amount = if let State::Number(x) = &objects[1] {x} else {"$0.00"};
            (Icons::Bitcoin, format!("You sent {}", amount))
        };
        
        Form::new(theme, vec![
            FormItem::text("Bitcoin address"), //, |a: String| Address::is_valid_address(a)),
            FormItem::number("Bitcoin amount", NumberVariant::Currency), //, |v: f32| Wallet::contains_amount(v)), // change to NumberVariant::Currency
            FormItem::enumerator("Transaction speed", vec![
                ("Standard", "Arrives in ~2 hours\n$0.18 bitcoin network fee"),
                ("Priority", "Arrives in ~30 minutes\n$0.32 bitcoin network fee"),
            ]),
        ], Some(Review::new("Confirm send", review)), Some(Success::new("Bitcoin sent", success)), closure)
    }
}

#[derive(Debug, Clone)]
pub struct MessagesHome;
impl MessagesHome {
    fn new(theme: &Theme) -> PageType {
        let messages = Message::tests();
        let message = messages[0].clone();
        let chat = vec![Screen::new_builder(theme, Chat::new(messages))];
        let items = vec![ListItem::avatar(message.author.avatar(), &message.author.name, &message.message, None, Some(Flow::new(chat)))];

        let new_message = NewMessageFlow::new(theme);
        Root::new("Messages",
            vec![Display::list(None, items, None)],
            None, ("New Message".into(), Flow::from_form(new_message)), None,
        )
    }
}


pub struct NewMessageFlow;
impl NewMessageFlow {
    pub fn new(theme: &Theme) -> Form {
        let closure = Box::new(move |_ctx: &mut Context, objects: &Vec<State>| {
            println!("New Message {:?}", objects);
        }) as Box<dyn FormSubmit>;

        let items = Profile::more_tests().into_iter().map(|profile| ListItem::avatar(profile.avatar(), &profile.name, "did address here", None, None)).collect::<Vec<_>>();
        Form::new(theme, vec![FormItem::search("Select recipient", items)], None, None, closure)
    }
}


pub struct Chat;
impl Chat {
    pub fn new(messages: Vec<Message>) -> Box<dyn PageBuilder> {
        Box::new(move |_builder: &Theme| {
            let profiles = messages.clone().into_iter().map(|m| m.author).collect::<HashSet<_>>().into_iter().collect::<Vec<_>>();
            PageType::messaging(messages.clone(), profiles)
        })
    }
}