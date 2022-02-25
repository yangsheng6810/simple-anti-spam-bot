use log;
use teloxide::Bot;
use teloxide::types::Message;
use teloxide::prelude::*;
use anyhow::Result;

struct MyBotHandler {}
 
impl<R> teloxide::dispatching::DispatcherHandler<R, Message> for MyBotHandler
where
    R: Send + 'static,
{
    fn handle(
        self,
        rx: DispatcherHandlerRx<R, Message>,
    ) -> futures_util::future::BoxFuture<'static, ()>
    where
        UpdateWithCx<R, Message>: Send + 'static,
    {
        return futures_util::FutureExt::boxed(bot_handle_message(rx));
    }
}
 
async fn bot_handle_message<R>(rx: DispatcherHandlerRx<R, Message>)
where
    R: Send + 'static,
{
    tokio_stream::wrappers::UnboundedReceiverStream::new(rx)
        .for_each_concurrent(None, |message| async move {
            dbg!(message.update);
        })
        .await;
}

#[tokio::main]
pub async fn main() -> Result<()> {
    log::info!("Starting bot thread...");
    let bot = Bot::new("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx")
        .parse_mode(teloxide::types::ParseMode::MarkdownV2);
 
    let message_handler = MyBotHandler {};
    Dispatcher::new(bot)
        .messages_handler(message_handler)
        .dispatch()
        .await;
    return Ok(());
}
