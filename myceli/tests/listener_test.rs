mod utils;

use messages::{ApplicationAPI, DataProtocol, Message};
use std::thread::sleep;
use std::time::Duration;
use utils::{TestController, TestListener};

#[test]
pub fn test_verify_listener_alive() {
    let listener = TestListener::new();
    listener.start().unwrap();

    let mut controller = TestController::new();

    let response =
        controller.send_and_recv(&listener.listen_addr, Message::request_available_blocks());

    assert_eq!(response, Message::available_blocks(vec![]));
}

#[test]
pub fn test_transmit_receive_block() {
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
        Message::transmit_block(&root_cid, &receiver.listen_addr),
        &transmitter.listen_addr,
    );

    utils::wait_receiving_done(&receiver, &mut controller);

    let resp = controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks());

    assert_eq!(resp, Message::available_blocks(vec![root_cid]));
}

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

#[test]
pub fn test_import_transmit_export_file() {
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

    let export_path = format!("{}/export", &receiver.test_dir.to_str().unwrap());
    controller.send_msg(
        Message::export_dag(&root_cid, &export_path),
        &receiver.listen_addr,
    );

    sleep(Duration::from_millis(100));

    let imported_hash = utils::hash_file(&test_file_path);
    let exported_hash = utils::hash_file(&export_path);
    assert_eq!(imported_hash, exported_hash);
}

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

#[test]
pub fn test_transmit_dag_no_response_exceed_retries() {
    let transmitter = TestListener::new();
    let mut controller = TestController::new();

    let controller_addr = controller
        .transport
        .socket
        .local_addr()
        .unwrap()
        .to_string();

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

    let retry_attempts = 5;

    controller.send_msg(
        Message::transmit_dag(&root_cid, &controller_addr, retry_attempts),
        &transmitter.listen_addr,
    );

    let mut retries = 0;

    loop {
        match controller.recv_msg() {
            // These are expected prior to getting the missing dag block requests
            Ok(Message::DataProtocol(DataProtocol::Block(_))) => {}
            Ok(Message::DataProtocol(DataProtocol::RequestMissingDagWindowBlocks {
                cid,
                blocks: _,
            })) => {
                assert_eq!(cid, root_cid);
                retries += 1;
            }
            _ => {
                break;
            }
        }
    }

    // A RequestMissingDagBlocks is sent immediately after a dag transmission, and then
    // once again for each retry attempt, so we should expect retry_attempts+1
    assert_eq!(retries, retry_attempts + 1);
}

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

#[test]
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
