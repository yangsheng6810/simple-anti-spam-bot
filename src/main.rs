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
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufRead, BufReader};

use tracing::{trace, debug, info, warn, error, span, Level, Instrument};
use tracing_subscriber;

static SPAM: OnceCell<HashSet<String>> = OnceCell::new();

static BOT_NAME: &str = "simple_anti_spam_bot";

async fn is_spam(ss: &str, lock: Arc<RwLock<HashSet<String>>>) -> bool {
    // let spam_db = SPAM.get().unwrap().clone();
    let spam_db = lock.read().await;
    for spam_str in &*spam_db {
        trace!("Testing against spam str: {:?}", spam_str);
        if ss.contains(spam_str) {
            warn!("SPAM found against {:?}! Text is {:?}", &spam_str, &ss);
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
    let chat_id = message.chat.id.clone();
    let group_title = message.chat.title();
    let group_span = span!(Level::INFO, "group", id = &chat_id, name = &group_title);
    async {
        if let MessageKind::Common(msg) = message.kind.clone() {
            if let MediaKind::Text(msg_text) = msg.media_kind {
                trace!("text is {:?}", &msg_text.text);
                if msg_text.text.len() > 100 {
                    debug!("text is {:?}", &msg_text.text);
                }
                let content = msg_text.text.clone();
                if is_spam(&content, lock).await {
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
    }.instrument(group_span).await;
}

async fn send_msg_auto_delete(bot: AutoSend<Bot>, msg: Message, ss: &str) {
    let check_wait_duration = tokio::time::Duration::from_secs(30);
    match bot.delete_message(msg.chat.id, msg.id).await {
        Ok(_) => {
            trace!("User command deleted successfully");
        },
        Err(e) => {
            debug!("Failed to delete user command due to {:?}", &e);
        }
    }
    match bot.send_message(msg.chat.id, ss).await {
        Ok(msg) => {
            let chat_id = msg.chat.id;
            let msg_id = msg.id;
            tokio::spawn(async move {
                tokio::time::sleep(check_wait_duration).await;
                match bot.delete_message(chat_id, msg_id).await {
                    Ok(_) => {
                        trace!("Message deleted successfully");
                    },
                    Err(e) => {
                        warn!("Message delete failed due to {:?}", &e);
                    }
                }
            }.in_current_span()); // prepare the current span to make tracing happy
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
        .branch(Update::filter_message()
            // Filter a maintainer by a used ID.
                .filter_command::<AdminCommand>()
                .endpoint(
                    |msg: Message, bot: AutoSend<Bot>, cmd: AdminCommand, lock: Arc<RwLock<HashSet<String>>>| async move {
                        let group_id = msg.chat.id;
                        let group_title = msg.chat.title();

                        let username: Option<&str>;
                        let user = match msg.from() {
                            Some(user) => {
                                username = Some(&user.first_name);
                                Some(user.id)
                            },
                            _ => {
                                username = None;
                                None
                            }
                        };
                        let group_span = span!(Level::INFO, "command",
                                               id = &group_id,
                                               name = &group_title,
                                               by = &user,
                                               username = &username);

                        async {
                            info!("Received command {:?}", &cmd);
                            if !is_from_admin(&msg, &bot).await {
                                warn!("Not from admin");
                                return
                            }
                            trace!("Command is from an admin");
                            match &msg.kind {
                                MessageKind::Common(msg) => {
                                    if let MediaKind::Text(msg) = &msg.media_kind {
                                        let text = &msg.text;
                                        let bot_name_str =  format!("@{}", &BOT_NAME);
                                        // debug!("text {} contains {} gets {}", &text, &bot_name_str, text.contains(&bot_name_str));
                                        if !text.contains(&bot_name_str) {
                                            return
                                        }
                                    } else {
                                        return
                                    }
                                },
                                _ => {
                                    return
                                }
                            }
                            let mut final_msg;
                            match cmd {
                                AdminCommand::Add(ss) => {
                                    let ss = ss.trim();
                                    if ss.is_empty() {
                                        final_msg = format!("Input is empty");
                                    } else {
                                        if ss.len() < 3 {
                                            final_msg = format!("SPAM phrase needs to be at least 3 bytes long");
                                        } else {
                                            {
                                                let mut spam_db = lock.write().await;
                                                spam_db.insert(String::from(ss));
                                                debug!("Updated SPAM database is {:?}", *spam_db);
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
                                            final_msg = format!("SPAM phrarse \"{}\" removed from the database", &ss);
                                            info!("SPAM phrase {} removed from the database", &ss);
                                        } else {
                                            final_msg = format!("SPAM phrase \"{}\" not found in the database. \
                                                                 Use \"/print\" to show a list of spam phrases", &ss);
                                            info!("No SPAM phras was removed from the database");
                                        }
                                    }
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
                        }.instrument(group_span).await;

                        Ok(())
                    },
                ))
        .branch(Update::filter_edited_message().endpoint(
            |update: Update, bot: AutoSend<Bot>, lock: Arc<RwLock<HashSet<String>>>| async move {
                trace!("Received a message edit.");
                if let UpdateKind::EditedMessage(message) = update.kind {
                    handle_message(&message, &bot, lock).await;
                }
                respond(())
            }
        ))
        .branch(Update::filter_message().endpoint(
            |update: Update, bot: AutoSend<Bot>, lock: Arc<RwLock<HashSet<String>>>| async move {
                trace!("Received a normal message.");
                if let UpdateKind::Message(message) = update.kind {
                    handle_message(&message, &bot, lock).await;
                }
                respond(())
            }
        ));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![spam_set_lock.clone()])
        .build()
        .setup_ctrlc_handler()
        .dispatch().await;

    save_database(&*spam_set_lock.read().await);
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
        debug!("spam string {:?} added to database", &spam_str);
        spam_db.insert(String::from(spam_str));
    }
    // only set once, so will never fail
    SPAM.set(spam_db).unwrap();
}

fn get_env() {
    info!("loading env");
    get_spam_from_env();
}

async fn is_from_admin(message: &Message, bot: &AutoSend<Bot>) -> bool {
    if let Some(user) = message.from() {
         match bot.get_chat_member(message.chat.id, user.id).send().await {
            Ok(ChatMember{user:_, kind}) => {
                return kind.is_privileged();
            }
            Err(e) => {
                debug!("get error {:?}", &e)
            }
        }
    }
    false
}

fn save_database(spam_db: &HashSet<String>) {
    let file_name = String::from("env.sh");
    let env_key = "SPAM_STR";
    let starts_with_str = format!("export {}=", &env_key);

    let mut final_str = String::from("");
    let spam_str = spam_db.clone().into_iter().collect::<Vec<String>>().join(":");
    let mut read_successfully = false;
    match File::open(&file_name) {
        Ok(f) => {
            let f = BufReader::new(f);
            for line in f.lines() {
                match line {
                    Ok(line) => if line.starts_with(&starts_with_str) {
                        final_str.push_str(format!("export {}=\"{}\"", &env_key, &spam_str).as_str())
                    } else {
                        final_str.push_str(format!("{}\n", &line).as_str());
                    },
                    Err(e) => error!("Error reading file: {}", e)
                }
            }
            read_successfully = true;
            // println!("{}", final_str);
        },
        Err(e) => {
            error!("Failed to read file: {}", e);
        }
    }
    if read_successfully {
        // Open a file in write-only mode, returns `io::Result<File>`
        match File::create(&file_name) {
            Ok(mut file) => {
                match file.write_all(final_str.as_bytes()) {
                    Err(why) => {
                        error!("Couldn't write to {}: {}", &file_name, why);
                        error!("May need to update manually: {}", &final_str);
                    },
                    Ok(_) => info!("{} updated successfully", &file_name),
                }
            },
            Err(why) => error!("Couldn't create {}: {}", &file_name, why),
        };
    } else {
        error!("May need to update {} manually: {}", &file_name, &final_str);
    }
}
