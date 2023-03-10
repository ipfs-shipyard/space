mod utils;

use messages::{ApplicationAPI, Message};
use std::time::Duration;
use tokio::time::sleep;
use utils::{TestController, TestListener};

#[tokio::test]
pub async fn test_verify_listener_alive() {
    let listener = TestListener::new("127.0.0.1:8080");
    listener.start().await.unwrap();

    let mut controller = TestController::new().await;

    let response = controller
        .send_and_recv(&listener.listen_addr, Message::request_available_blocks())
        .await
        .unwrap();

    assert_eq!(response, Message::available_blocks(vec![]));
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_transmit_receive_block() {
    let transmitter = TestListener::new("127.0.0.1:8080");
    let receiver = TestListener::new("127.0.0.1:8081");
    let mut controller = TestController::new().await;

    transmitter.start().await.unwrap();
    receiver.start().await.unwrap();

    let test_file_path = transmitter.generate_file().unwrap();
    let resp = controller
        .send_and_recv(
            &transmitter.listen_addr,
            Message::import_file(&test_file_path),
        )
        .await
        .unwrap();
    let root_cid = match resp {
        Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }) => cid,
        other => panic!("Failed to receive FileImported msg {other:?}"),
    };

    controller
        .send_msg(
            Message::transmit_block(&root_cid, &receiver.listen_addr),
            &transmitter.listen_addr,
        )
        .await
        .unwrap();

    sleep(Duration::from_millis(100)).await;

    let resp = controller
        .send_and_recv(&receiver.listen_addr, Message::request_available_blocks())
        .await
        .unwrap();

    assert_eq!(resp, Message::available_blocks(vec![root_cid]));
}
