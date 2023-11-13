mod utils;

#[allow(unused)]
use messages::{ApplicationAPI, Message};
use std::thread::sleep;
use std::time::Duration;
use utils::{TestController, TestListener};

#[test]
pub fn test_verify_listener_alive() {
    // env_logger::init();
    let listener = TestListener::new();
    listener.start().unwrap();

    let mut controller = TestController::new();

    let response =
        controller.send_and_recv(&listener.listen_addr, Message::request_available_blocks());

    assert_eq!(response, Message::available_blocks(vec![]));
}

#[cfg(feature = "proto_ship")]
#[test]
pub fn test_transmit_receive_dag() {
    let transmitter = TestListener::new();
    let receiver = TestListener::new();
    let mut controller = TestController::new();

    transmitter.start().unwrap();
    receiver.start().unwrap();

    let test_file_path = transmitter.generate_file().unwrap();
    let resp = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::import_file(&test_file_path),
    );
    let root_cid = match resp {
        Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }) => cid,
        other => panic!("Failed to receive FileImported msg {other:?}"),
    };

    controller.send_msg(
        Message::transmit_dag(&root_cid, &receiver.listen_addr, 0),
        &transmitter.listen_addr,
    );

    utils::wait_receiving_done(&receiver, &mut controller);

    let receiver_blocks =
        controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks());

    let transmitter_blocks = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::request_available_blocks(),
    );

    assert_eq!(receiver_blocks, transmitter_blocks);
}

#[cfg(feature = "proto_ship")]
#[test]
pub fn test_transmit_receive_dag_with_retries() {
    let transmitter = TestListener::new();
    let receiver = TestListener::new();
    let mut controller = TestController::new();

    transmitter.start().unwrap();
    receiver.start().unwrap();

    let test_file_path = transmitter.generate_file().unwrap();
    let resp = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::import_file(&test_file_path),
    );
    let root_cid = match resp {
        Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }) => cid,
        other => panic!("Failed to receive FileImported msg {other:?}"),
    };

    controller.send_msg(
        Message::transmit_dag(&root_cid, &receiver.listen_addr, 5),
        &transmitter.listen_addr,
    );

    utils::wait_receiving_done(&receiver, &mut controller);

    let receiver_blocks =
        controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks());

    let transmitter_blocks = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::request_available_blocks(),
    );

    assert_eq!(receiver_blocks, transmitter_blocks);
}

#[cfg(feature = "proto_ship")]
#[test]
pub fn test_compare_dag_list_after_transfer() {
    let transmitter = TestListener::new();
    let receiver = TestListener::new();
    let mut controller = TestController::new();

    transmitter.start().unwrap();
    receiver.start().unwrap();

    let test_file_path = transmitter.generate_file().unwrap();
    let resp = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::import_file(&test_file_path),
    );
    let root_cid = match resp {
        Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }) => cid,
        other => panic!("Failed to receive FileImported msg {other:?}"),
    };

    controller.send_msg(
        Message::transmit_dag(&root_cid, &receiver.listen_addr, 0),
        &transmitter.listen_addr,
    );

    utils::wait_receiving_done(&receiver, &mut controller);

    let transmitter_dags = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::ApplicationAPI(ApplicationAPI::RequestAvailableDags),
    );

    let receiver_dags = controller.send_and_recv(
        &receiver.listen_addr,
        Message::ApplicationAPI(ApplicationAPI::RequestAvailableDags),
    );

    assert_eq!(transmitter_dags, receiver_dags);
}

#[cfg(feature = "proto_ship")]
#[ignore]
#[test]
pub fn test_resume_dag_after_reconnect() {
    let transmitter = TestListener::new();
    let receiver = TestListener::new();
    let mut controller = TestController::new();

    transmitter.start().unwrap();
    receiver.start().unwrap();

    let test_file_path = transmitter.generate_file().unwrap();
    let resp = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::import_file(&test_file_path),
    );
    let root_cid = match resp {
        Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }) => cid,
        other => panic!("Failed to receive FileImported msg {other:?}"),
    };

    controller.send_msg(
        Message::ApplicationAPI(ApplicationAPI::SetConnected { connected: false }),
        &transmitter.listen_addr,
    );

    controller.send_msg(
        Message::transmit_dag(&root_cid, &receiver.listen_addr, 0),
        &transmitter.listen_addr,
    );

    utils::wait_receiving_done(&receiver, &mut controller);

    let receiver_blocks =
        controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks());

    assert_eq!(
        receiver_blocks,
        Message::ApplicationAPI(ApplicationAPI::AvailableBlocks { cids: vec![] })
    );

    controller.send_msg(
        Message::ApplicationAPI(ApplicationAPI::SetConnected { connected: true }),
        &transmitter.listen_addr,
    );

    utils::wait_receiving_done(&receiver, &mut controller);

    let receiver_blocks =
        controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks());

    let transmitter_blocks = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::request_available_blocks(),
    );

    assert_eq!(receiver_blocks, transmitter_blocks);
}

#[test]
pub fn test_no_transmit_after_disconnect() {
    let transmitter = TestListener::new();
    let receiver = TestListener::new();
    let mut controller = TestController::new();

    transmitter.start().unwrap();
    receiver.start().unwrap();

    let test_file_path = transmitter.generate_file().unwrap();
    let resp = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::import_file(&test_file_path),
    );
    let root_cid = match resp {
        Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }) => cid,
        other => panic!("Failed to receive FileImported msg {other:?}"),
    };

    controller.send_msg(
        Message::ApplicationAPI(ApplicationAPI::SetConnected { connected: false }),
        &transmitter.listen_addr,
    );

    controller.send_msg(
        Message::transmit_dag(&root_cid, &receiver.listen_addr, 0),
        &transmitter.listen_addr,
    );

    utils::wait_receiving_done(&receiver, &mut controller);

    let receiver_blocks =
        controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks());

    assert_eq!(
        receiver_blocks,
        Message::ApplicationAPI(ApplicationAPI::AvailableBlocks { cids: vec![] })
    );
}

#[cfg(feature = "proto_ship")]
#[test]
#[ignore]
pub fn test_transmit_resume_after_timeout() {
    let transmitter = TestListener::new();
    let receiver = TestListener::new();
    let mut controller = TestController::new();

    transmitter.start().unwrap();

    let test_file_path = transmitter.generate_file().unwrap();
    let resp = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::import_file(&test_file_path),
    );
    let root_cid = match resp {
        Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }) => cid,
        other => panic!("Failed to receive FileImported msg {other:?}"),
    };

    controller.send_msg(
        Message::transmit_dag(&root_cid, &receiver.listen_addr, 1),
        &transmitter.listen_addr,
    );

    sleep(Duration::from_secs(1));

    receiver.start().unwrap();

    let receiver_blocks =
        controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks());

    assert_eq!(
        receiver_blocks,
        Message::ApplicationAPI(ApplicationAPI::AvailableBlocks { cids: vec![] })
    );

    controller.send_msg(
        Message::ApplicationAPI(ApplicationAPI::ResumeTransmitDag { cid: root_cid }),
        &transmitter.listen_addr,
    );

    utils::wait_receiving_done(&receiver, &mut controller);

    let receiver_blocks =
        controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks());

    let transmitter_blocks = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::request_available_blocks(),
    );

    assert_eq!(receiver_blocks, transmitter_blocks);
}

// TODO: need another test here to verify single-block transfers, they seem to have some issues that multi-block files don't have
