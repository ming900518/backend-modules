use crate::AppState;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use base_library::new_uuid_v1;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::collections::HashSet;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use uuid::Uuid;

// Modified from https://github.com/tokio-rs/axum/blob/main/examples/chat/src/main.rs
// Add: Chat Room Support, Redis Cache Support (NDY).

#[derive(Deserialize)]
struct FirstMsg {
    user_uuid: Uuid,
    chatroom_uuid: Option<Uuid>,
    _latest_msg_id: Option<String>,
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

async fn websocket(stream: WebSocket, state: Arc<Mutex<AppState>>) {
    // By splitting we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();

    let mut chatroom_uuid = String::new();

    let mut user_uuid = String::new();

    let mut connection = state.try_lock().unwrap().pool.get().unwrap();

    // Loop until a text message is found.
    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(first_msg) = message {
            match serde_json::from_str::<FirstMsg>(&first_msg) {
                Ok(first_msg) => {
                    chatroom_uuid = if first_msg.chatroom_uuid.is_none() {
                        new_uuid_v1().to_string()
                    } else {
                        first_msg.chatroom_uuid.unwrap().to_string()
                    };

                    user_uuid = first_msg.user_uuid.to_string();

                    redis::cmd("xadd")
                        .arg("chatroom")
                        .arg("*")
                        .arg("id")
                        .arg(&chatroom_uuid)
                        .query::<String>(&mut connection)
                        .unwrap();

                    redis::cmd("xadd")
                        .arg(format!("chatroom_users:{}", &chatroom_uuid))
                        .arg("*")
                        .arg("users")
                        .arg(&user_uuid)
                        .query::<String>(&mut connection)
                        .unwrap();

                    prepare_connection(state.deref(), &chatroom_uuid, &user_uuid);

                    sender
                        .send(Message::Text(format!(
                            "Welcome {}! Your chatroom_uuid is {}.",
                            &user_uuid, &chatroom_uuid
                        )))
                        .await
                        .unwrap();

                    break;
                }
                Err(rejection) => {
                    sender
                        .send(Message::Text(rejection.to_string()))
                        .await
                        .unwrap();
                    return;
                }
            }
        }
    }

    let cloned_chatroom_uuid = chatroom_uuid.clone();
    let cloned_user_uuid = user_uuid.clone();

    println!("{cloned_chatroom_uuid}");
    println!("{:#?}", state.try_lock().unwrap().tx);

    // Subscribe before sending joined message.
    let mut rx = state
        .try_lock()
        .unwrap()
        .tx
        .try_lock()
        .unwrap()
        .get(&cloned_chatroom_uuid)
        .unwrap()
        .subscribe();

    // Send joined message to all subscribers.
    let msg = format!("{} joined.", user_uuid);
    tracing::debug!("{}", msg);
    println!("{:#?}", state.try_lock().unwrap().tx.try_lock().unwrap());
    let _ = state
        .try_lock()
        .unwrap()
        .tx
        .try_lock()
        .unwrap()
        .get(&*cloned_chatroom_uuid)
        .unwrap()
        .send(msg);

    // This task will receive broadcast messages and send text message to our client.
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // In any websocket error, break loop.
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // This task will receive messages from client and send them to broadcast subscribers.
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            // Add username before message.
            let _ = state
                .try_lock()
                .unwrap()
                .tx
                .try_lock()
                .unwrap()
                .get(&*cloned_chatroom_uuid)
                .unwrap()
                .send(format!("{}: {}", cloned_user_uuid, text));
        }
    });

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }
}

fn prepare_connection(state: &Mutex<AppState>, chatroom_uuid: &String, user_uuid: &str) {
    let state_locked = &mut state.try_lock().unwrap();
    let mut chatroom_set = state_locked.chatroom_set.try_lock().unwrap();
    let mut new_user_set = match chatroom_set.get(chatroom_uuid) {
        None => {
            let (tx, _rx) = broadcast::channel(100);
            let state_tx = &mut state_locked.tx.try_lock().unwrap();
            state_tx.insert(chatroom_uuid.clone(), tx);
            HashSet::new()
        }
        Some(_) => chatroom_set.remove(chatroom_uuid).unwrap(),
    };
    new_user_set.insert(user_uuid.parse().unwrap());
    chatroom_set.insert(chatroom_uuid.clone(), new_user_set);
    println!("{:#?}", chatroom_set);
}
