mod utils;

use messages::{ApplicationAPI, Message};
use std::time::Duration;
use tokio::time::sleep;
use utils::{TestController, TestListener};

#[tokio::test]
pub async fn test_verify_listener_alive() {
    let listener = TestListener::new();
    listener.start().await.unwrap();

    let mut controller = TestController::new().await;

    let response = controller
        .send_and_recv(&listener.listen_addr, Message::request_available_blocks())
        .await;

    assert_eq!(response, Message::available_blocks(vec![]));
}

#[tokio::test]
pub async fn test_transmit_receive_block() {
    let transmitter = TestListener::new();
    let receiver = TestListener::new();
    let mut controller = TestController::new().await;

    transmitter.start().await.unwrap();
    receiver.start().await.unwrap();

    let test_file_path = transmitter.generate_file().unwrap();
    let resp = controller
        .send_and_recv(
            &transmitter.listen_addr,
            Message::import_file(&test_file_path),
        )
        .await;
    let root_cid = match resp {
        Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }) => cid,
        other => panic!("Failed to receive FileImported msg {other:?}"),
    };

    controller
        .send_msg(
            Message::transmit_block(&root_cid, &receiver.listen_addr),
            &transmitter.listen_addr,
        )
        .await;

    sleep(Duration::from_millis(100)).await;

    let resp = controller
        .send_and_recv(&receiver.listen_addr, Message::request_available_blocks())
        .await;

    assert_eq!(resp, Message::available_blocks(vec![root_cid]));
}

#[tokio::test]
pub async fn test_transmit_receive_dag() {
    let transmitter = TestListener::new();
    let receiver = TestListener::new();
    let mut controller = TestController::new().await;

    transmitter.start().await.unwrap();
    receiver.start().await.unwrap();

    let test_file_path = transmitter.generate_file().unwrap();
    let resp = controller
        .send_and_recv(
            &transmitter.listen_addr,
            Message::import_file(&test_file_path),
        )
        .await;
    let root_cid = match resp {
        Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }) => cid,
        other => panic!("Failed to receive FileImported msg {other:?}"),
    };

    controller
        .send_msg(
            Message::transmit_dag(&root_cid, &receiver.listen_addr),
            &transmitter.listen_addr,
        )
        .await;

    sleep(Duration::from_millis(50)).await;

    let receiver_blocks = controller
        .send_and_recv(&receiver.listen_addr, Message::request_available_blocks())
        .await;

    let transmitter_blocks = controller
        .send_and_recv(
            &transmitter.listen_addr,
            Message::request_available_blocks(),
        )
        .await;

    assert_eq!(receiver_blocks, transmitter_blocks);
}

#[tokio::test]
pub async fn test_import_transmit_export_file() {
    let transmitter = TestListener::new();
    let receiver = TestListener::new();
    let mut controller = TestController::new().await;

    transmitter.start().await.unwrap();
    receiver.start().await.unwrap();

    let test_file_path = transmitter.generate_file().unwrap();
    let resp = controller
        .send_and_recv(
            &transmitter.listen_addr,
            Message::import_file(&test_file_path),
        )
        .await;
    let root_cid = match resp {
        Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }) => cid,
        other => panic!("Failed to receive FileImported msg {other:?}"),
    };

    controller
        .send_msg(
            Message::transmit_dag(&root_cid, &receiver.listen_addr),
            &transmitter.listen_addr,
        )
        .await;

    sleep(Duration::from_millis(50)).await;

    let export_path = format!("{}/export", &receiver.test_dir.to_str().unwrap());
    controller
        .send_msg(
            Message::export_dag(&root_cid, &export_path),
            &receiver.listen_addr,
        )
        .await;

    sleep(Duration::from_millis(50)).await;

    let imported_hash = utils::hash_file(&test_file_path);
    let exported_hash = utils::hash_file(&export_path);
    assert_eq!(imported_hash, exported_hash);
}
