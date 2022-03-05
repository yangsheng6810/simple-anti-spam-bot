// This bot answers how many messages it received in total on every message.
use teloxide::{
    prelude2::*,
    dispatching2::UpdateFilterExt
};
use teloxide::types::{
    UpdateKind,
    MessageKind,
    MediaKind,
    ChatMember,
};
use teloxide::utils::command::BotCommand;

use tokio::sync::RwLock;
use std::sync::Arc;
use once_cell::sync::OnceCell;
use std::collections::HashSet;
use std::env;

use tracing::{trace, debug, info, warn};
use tracing_subscriber;

static ADMIN: OnceCell<HashSet<i64>> = OnceCell::new();
static SPAM: OnceCell<HashSet<String>> = OnceCell::new();

static BOT_NAME: &str = "simple_anti_spam_bot";

async fn is_spam(ss: &str, lock: Arc<RwLock<HashSet<String>>>) -> bool {
    // let spam_db = SPAM.get().unwrap().clone();
    let spam_db = lock.read().await;
    for spam_str in &*spam_db {
        trace!("Testing against spam str: {:?}", spam_str);
        if ss.contains(spam_str) {
            return true;
        }
    }
    false
}

#[derive(BotCommand, Clone, Debug)]
#[command(rename = "lowercase", description = "Admin commands")]
enum AdminCommand {
    #[command(description = "Add a new blocked phrase")]
    Add(String),
    #[command(description = "Remove an existing blocked phrase")]
    Remove(String),
    #[command(description = "Print current blocked phrases")]
    Print,
    #[command(description = "Show help")]
    Help,
}

async fn handle_message(message: &Message, bot: &AutoSend<Bot>, lock: Arc<RwLock<HashSet<String>>>) {
    let message_id = message.id.clone();
    debug!("message id {:?}", &message_id);
    let chat_id = message.chat.id.clone();
    debug!("chat id {:?}", &chat_id);
    if let MessageKind::Common(msg) = message.kind.clone() {
        if let MediaKind::Text(msg_text) = msg.media_kind {
            debug!("text is {:?}", &msg_text.text);
            let content = msg_text.text.clone();
            if is_spam(&content, lock).await {
                warn!("SPAM found and deleted! Text is {:?}", &msg_text.text);
                match bot.delete_message(chat_id, message_id).await {
                    Ok(_) => warn!("Message {:?} deleted", &message_id),
                    Err(e) => info!("Delete message {:?} failed with error {:?}", &message_id, &e)
                }
                if let Some(user_id) = msg.from {
                    match bot.kick_chat_member(chat_id, user_id.id)
                             .revoke_messages(true).await
                    {
                        Ok(_) => warn!("User {:?} revoked", &user_id.id),
                        Err(e) => info!("Kick user {:?} failed with error {:?}", user_id, &e)
                    }
                } else {
                    warn!("could not find")
                }
            };
        }
    }
}


async fn send_msg_auto_delete(bot: AutoSend<Bot>, msg: Message, ss: &str) {
    let check_wait_duration = tokio::time::Duration::from_secs(30);
    match bot.delete_message(msg.chat.id, msg.id).await {
        Ok(_) => {
            debug!("User command deleted successfully");
        },
        Err(e) => {
            debug!("Failed to delete user command due to {:?}", &e);
        }
    }
    match bot.send_message(msg.chat.id, ss).await {
        Ok(msg) => {
            tokio::spawn(async move {
                tokio::time::sleep(check_wait_duration).await;
                match bot.delete_message(msg.chat.id, msg.id).await {
                    Ok(_) => {
                        info!("Message deleted successfully");
                    },
                    Err(e) => {
                        warn!("Message delete failed due to {:?}", &e);
                    }
                }
            });
        },
        Err(e) => {
            warn!("Message failed to send due to {:?}", &e);
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    get_env();
    let spam_set_lock = Arc::new(RwLock::new(SPAM.get().unwrap().clone()));
    info!("Starting {}...", &BOT_NAME);

    let bot = Bot::from_env().auto_send();


    let handler = dptree::entry()
        .branch(
            Update::filter_message()
            // Filter a maintainer by a used ID.
                .filter_command::<AdminCommand>()
                .endpoint(
                    |msg: Message, bot: AutoSend<Bot>, cmd: AdminCommand, lock: Arc<RwLock<HashSet<String>>>| async move {
                        debug!("cmd is {:?}", &cmd);
                        if !is_from_admin(&msg, &bot).await {
                            warn!("Not from admin");
                            return Ok(())
                        }
                        debug!("is admin");
                        match &msg.kind {
                            MessageKind::Common(msg) => {
                                if let MediaKind::Text(msg) = &msg.media_kind {
                                    let text = &msg.text;
                                    let bot_name_str =  format!("@{}", &BOT_NAME);
                                    debug!("text {} contains {} gets {}", &text, &bot_name_str, text.contains(&bot_name_str));
                                    if !text.contains(&bot_name_str) {
                                        return Ok(())
                                    }
                                } else {
                                    return Ok(())
                                }
                            },
                            _ => {
                                return Ok(())
                            }
                        }
                        let mut final_msg;
                        match cmd {
                            AdminCommand::Add(ss) => {
                                let ss = ss.trim();
                                if ss.is_empty() {
                                    final_msg = format!("Input is empty")
                                } else {
                                    if ss.len() < 3 {
                                        final_msg = format!("SPAM phrase needs to be at least 3 bytes long");
                                    } else {
                                        {
                                            let mut w = lock.write().await;
                                            w.insert(String::from(ss));
                                            debug!("w is {:?}", *w);
                                        }
                                        info!("SPAM phrase {} added to the database", &ss);
                                        final_msg = format!("SPAM phrarse \"{}\" added to the database.", &ss);
                                    }
                                }
                            }
                            AdminCommand::Remove(ss) => {
                                {
                                    let mut w = lock.write().await;
                                    if w.contains(&ss) {
                                        w.remove(&ss);
                                        final_msg = format!("SPAM phrarse \"{}\" removed from the database", &ss)
                                    } else {
                                        final_msg = format!("SPAM phrase \"{}\" not found in the database. \
                                                             Use \"\\print\" to show a list of spam phrases", &ss);
                                    }
                                }
                                info!("SPAM phrase {} removed from the database", &ss);
                            }
                            AdminCommand::Help => {
                                info!("Handling help request");
                                final_msg = AdminCommand::descriptions();
                            }
                            AdminCommand::Print => {
                                info!("Handling print request");
                                final_msg = String::from("The list of spam phrases are:\n");
                                let mut count = 1;
                                {
                                    let spam_db = lock.read().await;
                                    for spam_str in &*spam_db {
                                        final_msg.push_str(format!("{:<3} \"{}\"\n", &count, &spam_str).as_str());
                                        count += 1;
                                    }
                                }
                            }
                        }
                        send_msg_auto_delete(bot, msg, &final_msg).await;
                        Ok(())
                    },
                ),
        )
        .branch(Update::filter_edited_message().endpoint(
            |update: Update, bot: AutoSend<Bot>, lock: Arc<RwLock<HashSet<String>>>| async move {
                debug!("Received a message edit.");
                if let UpdateKind::EditedMessage(message) = update.kind {
                    handle_message(&message, &bot, lock).await;
                }
                respond(())
            }
        ))
        .branch(Update::filter_message().endpoint(
            |update: Update, bot: AutoSend<Bot>, lock: Arc<RwLock<HashSet<String>>>| async move {
                debug!("Received a normal message.");
                if let UpdateKind::Message(message) = update.kind {
                    handle_message(&message, &bot, lock).await;
                }
                respond(())
            }
            ,))
        ;

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![spam_set_lock])
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
    info!("loading admin db");
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

async fn is_from_admin(message: &Message, bot: &AutoSend<Bot>) -> bool {
    if let Some(user) = message.from() {
         match bot.get_chat_member(message.chat_id(), user.id).send().await {
            Ok(ChatMember{_user, kind}) => {
                // debug!("user is {:?}", &user);
                debug!("kind is {:?}", &kind);
                return kind.is_privileged();
            }
            Err(e) => {
                debug!("get error {:?}", &e)
            }
        }
    }
    false
}
