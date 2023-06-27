use crate::utils::*;
use messages::{ApplicationAPI, Message};
use std::thread::sleep;
use std::time::Duration;

pub fn test_transmit_5kb_dag() {
    let (mut transmitter, mut receiver, mut controller) = testing_setup();

    transmitter.start().unwrap();
    receiver.start().unwrap();

    let test_file_path = transmitter.generate_file(1024 * 5).unwrap();
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

    wait_receiving_done(&receiver, &mut controller);

    let receiver_blocks =
        controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks());

    let transmitter_blocks = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::request_available_blocks(),
    );

    assert_eq!(receiver_blocks, transmitter_blocks);

    let receiver_validate_dag = controller.send_and_recv(
        &receiver.listen_addr,
        Message::ApplicationAPI(ApplicationAPI::ValidateDag {
            cid: root_cid.to_string(),
        }),
    );
    assert_eq!(
        receiver_validate_dag,
        Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse {
            cid: root_cid,
            result: "Dag is valid".to_string()
        })
    );
}

pub fn test_transmit_500kb_dag() {
    let (mut transmitter, mut receiver, mut controller) = testing_setup();

    transmitter.start().unwrap();
    receiver.start().unwrap();

    let test_file_path = transmitter.generate_file(1024 * 500).unwrap();
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

    wait_receiving_done(&receiver, &mut controller);

    let receiver_blocks =
        controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks());

    let transmitter_blocks = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::request_available_blocks(),
    );

    assert_eq!(receiver_blocks, transmitter_blocks);

    let receiver_validate_dag = controller.send_and_recv(
        &receiver.listen_addr,
        Message::ApplicationAPI(ApplicationAPI::ValidateDag {
            cid: root_cid.to_string(),
        }),
    );
    assert_eq!(
        receiver_validate_dag,
        Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse {
            cid: root_cid,
            result: "Dag is valid".to_string()
        })
    );
}

pub fn test_transmit_5mb_dag() {
    let (mut transmitter, mut receiver, mut controller) = testing_setup();

    transmitter.start().unwrap();
    receiver.start().unwrap();

    let test_file_path = transmitter.generate_file(1024 * 1024 * 5).unwrap();
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

    wait_receiving_done(&receiver, &mut controller);

    let receiver_blocks =
        controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks());

    let transmitter_blocks = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::request_available_blocks(),
    );

    assert_eq!(receiver_blocks, transmitter_blocks);

    let receiver_validate_dag = controller.send_and_recv(
        &receiver.listen_addr,
        Message::ApplicationAPI(ApplicationAPI::ValidateDag {
            cid: root_cid.to_string(),
        }),
    );
    assert_eq!(
        receiver_validate_dag,
        Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse {
            cid: root_cid,
            result: "Dag is valid".to_string()
        })
    );
}

pub fn test_transmit_20mb_dag_over_5s_passes() {
    let (mut transmitter, mut receiver, mut controller) = testing_setup();

    transmitter.start().unwrap();
    receiver.start().unwrap();

    let test_file_path = transmitter.generate_file(1024 * 1024 * 20).unwrap();
    let resp = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::import_file(&test_file_path),
    );
    let root_cid = match resp {
        Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }) => cid,
        other => panic!("Failed to receive FileImported msg {other:?}"),
    };

    // Begin transmission
    controller.send_msg(
        Message::transmit_dag(&root_cid, &receiver.listen_addr, 0),
        &transmitter.listen_addr,
    );

    // Begin pass cycle, capping out at 10 passes
    for _ in 1..10 {
        // Begin pass
        controller.send_msg(
            Message::ApplicationAPI(ApplicationAPI::SetConnected { connected: true }),
            &transmitter.listen_addr,
        );

        // wait for 5s pass
        sleep(Duration::from_secs(5));

        // End pass
        controller.send_msg(
            Message::ApplicationAPI(ApplicationAPI::SetConnected { connected: false }),
            &transmitter.listen_addr,
        );

        // wait 1s inter-pass period
        sleep(Duration::from_secs(1));

        // Check if transfer completed in prior pass
        if controller.send_and_recv(
            &receiver.listen_addr,
            Message::ApplicationAPI(ApplicationAPI::ValidateDag {
                cid: root_cid.to_string(),
            }),
        ) == Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse {
            cid: root_cid.to_string(),
            result: "Dag is valid".to_string(),
        }) {
            break;
        }
    }

    // Verify complete file was passes complete
    let receiver_blocks =
        controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks());

    let transmitter_blocks = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::request_available_blocks(),
    );

    assert_eq!(receiver_blocks, transmitter_blocks);

    let receiver_validate_dag = controller.send_and_recv(
        &receiver.listen_addr,
        Message::ApplicationAPI(ApplicationAPI::ValidateDag {
            cid: root_cid.to_string(),
        }),
    );
    assert_eq!(
        receiver_validate_dag,
        Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse {
            cid: root_cid,
            result: "Dag is valid".to_string()
        })
    );
}

pub fn test_transmit_20mb_dag_over_5s_passes_receiver_off() {
    let (mut transmitter, mut receiver, mut controller) = testing_setup();

    transmitter.start().unwrap();
    receiver.start().unwrap();

    let test_file_path = transmitter.generate_file(1024 * 1024 * 20).unwrap();
    let resp = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::import_file(&test_file_path),
    );
    let root_cid = match resp {
        Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }) => cid,
        other => panic!("Failed to receive FileImported msg {other:?}"),
    };

    // Begin transmission
    controller.send_msg(
        Message::transmit_dag(&root_cid, &receiver.listen_addr, 0),
        &transmitter.listen_addr,
    );

    // Begin pass cycle, capping out at 10 passes
    for _ in 1..10 {
        receiver.start().unwrap();
        // Begin pass
        controller.send_msg(
            Message::ApplicationAPI(ApplicationAPI::SetConnected { connected: true }),
            &transmitter.listen_addr,
        );

        // wait for 5s pass
        sleep(Duration::from_secs(5));

        // End pass
        controller.send_msg(
            Message::ApplicationAPI(ApplicationAPI::SetConnected { connected: false }),
            &transmitter.listen_addr,
        );

        // wait 1s inter-pass period
        sleep(Duration::from_secs(1));

        // Check if transfer completed in prior pass
        if controller.send_and_recv(
            &receiver.listen_addr,
            Message::ApplicationAPI(ApplicationAPI::ValidateDag {
                cid: root_cid.to_string(),
            }),
        ) == Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse {
            cid: root_cid.to_string(),
            result: "Dag is valid".to_string(),
        }) {
            break;
        }

        // Terminate listener
        controller.send_msg(
            Message::ApplicationAPI(ApplicationAPI::Terminate),
            &receiver.listen_addr,
        );
        receiver.stop();
    }

    // Verify complete file was passes complete
    let receiver_blocks =
        controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks());

    let transmitter_blocks = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::request_available_blocks(),
    );

    assert_eq!(receiver_blocks, transmitter_blocks);

    let receiver_validate_dag = controller.send_and_recv(
        &receiver.listen_addr,
        Message::ApplicationAPI(ApplicationAPI::ValidateDag {
            cid: root_cid.to_string(),
        }),
    );
    assert_eq!(
        receiver_validate_dag,
        Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse {
            cid: root_cid,
            result: "Dag is valid".to_string()
        })
    );
}

pub fn test_transmit_20mb_dag_over_5s_passes_transmitter_off() {
    let (mut transmitter, mut receiver, mut controller) = testing_setup();

    transmitter.start().unwrap();
    receiver.start().unwrap();

    let test_file_path = transmitter.generate_file(1024 * 1024 * 20).unwrap();
    println!("Sending import-file {} to transmitter", &test_file_path);
    let resp = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::import_file(&test_file_path),
    );
    let root_cid = match resp {
        Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }) => cid,
        other => panic!("Failed to receive FileImported msg {other:?}"),
    };

    // Begin transmission
    controller.send_msg(
        Message::transmit_dag(&root_cid, &receiver.listen_addr, 5),
        &transmitter.listen_addr,
    );

    sleep(Duration::from_secs(5));

    // Begin pass cycle, capping out at 5 passes
    for _ in 1..5 {
        // End pass by terminating transmitter and disconnecting receiver
        controller.send_msg(
            Message::ApplicationAPI(ApplicationAPI::Terminate),
            &transmitter.listen_addr,
        );
        transmitter.stop();

        controller.send_msg(
            Message::ApplicationAPI(ApplicationAPI::SetConnected { connected: false }),
            &receiver.listen_addr,
        );
        println!("Sending validate-dag");
        // Check if transfer completed in prior pass
        let resp = controller.send_and_recv(
            &receiver.listen_addr,
            Message::ApplicationAPI(ApplicationAPI::ValidateDag {
                cid: root_cid.to_string(),
            }),
        );
        println!("got validate dag resp {resp:?}");
        if resp
            == Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse {
                cid: root_cid.to_string(),
                result: "Dag is valid".to_string(),
            })
        {
            break;
        }

        // wait 1s inter-pass period
        sleep(Duration::from_secs(1));

        // Start next pass by starting transmitter and connecting receiver
        transmitter.start().unwrap();
        controller.send_msg(
            Message::ApplicationAPI(ApplicationAPI::SetConnected { connected: true }),
            &receiver.listen_addr,
        );
        controller.send_msg(
            Message::ApplicationAPI(ApplicationAPI::RequestResumeDagTransfer {
                cid: root_cid.to_string(),
                target_addr: transmitter.listen_addr.to_string(),
            }),
            &receiver.listen_addr,
        );

        // wait for 5s pass
        sleep(Duration::from_secs(5));
    }
    println!("Sending request-available-blocks to receiver");
    // Verify complete file was passes complete
    let receiver_blocks =
        controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks());
    println!("Sending request-available-blocks to transmitter");
    let transmitter_blocks = controller.send_and_recv(
        &transmitter.listen_addr,
        Message::request_available_blocks(),
    );

    // assert_eq!(receiver_blocks.len(), transmitter_blocks.len());
    println!("Sending validate-dag to receiver");
    let receiver_validate_dag = controller.send_and_recv(
        &receiver.listen_addr,
        Message::ApplicationAPI(ApplicationAPI::ValidateDag {
            cid: root_cid.to_string(),
        }),
    );
    assert_eq!(
        receiver_validate_dag,
        Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse {
            cid: root_cid,
            result: "Dag is valid".to_string()
        })
    );
}
