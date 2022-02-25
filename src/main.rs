// This bot answers how many messages it received in total on every message.
use teloxide::{prelude2::*, dispatching2::UpdateFilterExt};
use teloxide::types::UpdateKind::{EditedMessage};
use teloxide::types::MessageKind::{Common};
use teloxide::types::UpdateKind;
use teloxide::types::MediaKind::{Text};

use once_cell::sync::OnceCell;
use std::collections::HashSet;
use std::env;

use tracing::{trace, debug, info, warn};
use tracing_subscriber;

static ADMIN: OnceCell<HashSet<i64>> = OnceCell::new();
static SPAM: OnceCell<HashSet<String>> = OnceCell::new();

fn is_spam(ss: &str) -> bool {
    // if ss.contains("蚂蚁币") {
    //     return true;
    // }
    let spam_db = SPAM.get().unwrap().clone();
    for spam_str in spam_db {
        trace!("Testing against spam str: {:?}", &spam_str);
        if ss.contains(&spam_str) {
            return true;
        }
    }
    false
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    get_env();
    info!("Starting shared_state_bot...");

    let bot = Bot::from_env().auto_send();

    let handler = dptree::entry()
        .branch(Update::filter_edited_message().endpoint(
            |update: Update, bot: AutoSend<Bot>| async move {
                info!("Received a message edit.");
                // dbg!(&update);
                if let EditedMessage(message) = update.kind {
                    // dbg!(&msg.kind);
                    let message_id = message.id.clone();
                    debug!("message id {:?}", &message_id);
                    let chat_id = message.chat.id.clone();
                    debug!("chat id {:?}", &chat_id);
                    if let Common(msg) = message.kind {
                        // dbg!(&msg);
                        if let Text(msg_text) = msg.media_kind {
                            // dbg!(&msg);
                            debug!("text is {:?}", &msg_text.text);
                            let content = msg_text.text.clone();
                            if is_spam(&content) {
                                warn!("SPAM found!");
                                warn!("text is {:?}", &msg_text.text);
                                warn!("delete it!");
                                bot.delete_message(chat_id, message_id).await?;

                                if let Some(user_id) = msg.from {
                                    bot.kick_chat_member(
                                        chat_id,
                                        user_id.id
                                    )
                                       .revoke_messages(true)
                                     .await?;
                                } else {
                                    warn!("could not find")
                                }
                            }
                        }
                    }
                }
                // bot.send_message(msg.chat.id, "This is a group chat.").await?;
                respond(())
            }
        ))
        .branch(Update::filter_message().endpoint(
            |update: Update, bot: AutoSend<Bot>| async move {
                debug!("Received a normal message.");
                // dbg!(&update);
                if let UpdateKind::Message(message) = update.kind {
                    // dbg!(&msg.kind);
                    let message_id = message.id.clone();
                    debug!("message id {:?}", &message_id);
                    let chat_id = message.chat.id.clone();
                    debug!("chat id {:?}", &chat_id);
                    if let Common(msg) = message.kind {
                        // dbg!(&msg);
                        if let Text(msg_text) = msg.media_kind {
                            // dbg!(&msg);
                            debug!("text is {:?}", &msg_text.text);
                            let content = msg_text.text.clone();
                            if is_spam(&content) {
                                warn!("SPAM found!");
                                warn!("text is {:?}", &msg_text.text);
                                warn!("delete it!");
                                bot.delete_message(chat_id, message_id).await?;

                                if let Some(user_id) = msg.from {
                                    bot.kick_chat_member(
                                        chat_id,
                                        user_id.id
                                    )
                                       .revoke_messages(true)
                                     .await?;
                                } else {
                                    warn!("could not find")
                                }
                            }
                        }
                    }
                }
                // bot.send_message(msg.chat.id, "This is a group chat.").await?;
                respond(())
            }
            ,))
        ;

    Dispatcher::builder(bot, handler)
        .build()
        .setup_ctrlc_handler()
        .dispatch().await;
}

fn get_spam_from_env() {
    info!("loading spam db");
    let env_key = "SPAM_STR";
    let mut spam_db = HashSet::new();
    let spam_str = match env::var_os(&env_key) {
        Some(v) => v.into_string().unwrap(),
        None => {
            warn!("${} is not set", &env_key);
            String::from("0")
        }
    };
    for spam_str in spam_str.split(":"){
        info!("spam string {:?} added to database", &spam_str);
        spam_db.insert(String::from(spam_str));
    }
    // only set once, so will never fail
    SPAM.set(spam_db).unwrap();
}

#[allow(dead_code)]
fn get_admin_from_env() {
    let env_key = "ANTI_SPAM_BOT_ADMIN";
    let mut admin_db = HashSet::new();
    let admin_str = match env::var_os(&env_key) {
        Some(v) => v.into_string().unwrap(),
        None => {
            warn!("${} is not set", &env_key);
            String::from("0")
        }
    };
    for id in admin_str.split(":"){
        let admin = match id.parse::<i64>() {
            Ok(id) => id,
            Err(_) => 0
        };
        admin_db.insert(admin);
    }
    // only set once, so will never fail
    ADMIN.set(admin_db).unwrap();
}

fn get_env() {
    info!("loading env");
    // get_admin_from_env();
    get_spam_from_env();
}

#[allow(dead_code)]
fn is_admin(user_id: i64) -> bool {
    let admin_db = ADMIN.get().unwrap().clone();

    if admin_db.contains(&user_id) {
        info!("Admin {:?} confirmed", &user_id);
        return true
    }
    false
}
