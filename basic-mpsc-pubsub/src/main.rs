// TRY 1: Basic Pub/Sub Messaging Kernel Example
// Desmond Germans, Ph.D; www.germansmedia.nl

// This is rougly based on ROS Pub/Sub, after what was explored earlier.

// Notes:
// - Exclusively reactive modules based on message handlers alone might not be the way forward, there is also need for active loops and checks.
// - This implementation currently still misses a timer tick.
// - The M in MPSC is not particularly useful, since the broker might also want to send messages to a single module.

use {
    std::{
        time::Duration,
        thread,
        sync::mpsc::{
            Sender,
            Receiver,
            channel,
        },
        collections::HashSet,
        cell::RefCell,
    }
};

// The generic handler type
type Handler = fn(InMessage,&Sender<OutMessage>);

// These are messages to communicate between modules
#[derive(Clone)]
pub enum AppMessage {
    VideoFrame,  // hypothetical video frame
    FaceCoords,  // hypothetical face coordinates
}

// These are all possible messages into the module
pub enum InMessage {
    Startup,                     // initialize the module
    Shutdown,                    // shut down the module
    Tick,                        // timer tick signal (just for this example)
    Subscribed(String),          // indication that the module successfully subscribed to a topic
    Unsubscribed(String),        // indicating that the module successfully unsubscribed from a topic
    Generic(String,AppMessage),  // generic message from specific topic
}

// These are all possible messages out from the module, the API of the broker as seen by the module
pub enum OutMessage {
    Subscribe(String),           // "subscribe me to a topic"
    Unsubscribe(String),         // "unsubscribe me from a topic"
    Generic(String,AppMessage),  // "send AppMessage to a topic"
}

// A module
pub struct Module {
#[allow(dead_code)]
    name: String,                             // name of the module
    subscriptions: RefCell<HashSet<String>>,  // topics this module receiving from
    in_tx: Sender<InMessage>,                 // broker-side sender to this module
    out_rx: Receiver<OutMessage>,             // broker-side receiver from this module
}

// The broker owns the modules, so this API is from the broker's point of view
impl Module {
    fn new(name: &str,handler: Handler) -> Module {

        println!("Creating module '{}'",name);

        // create channel from broker to module
        let (in_tx,in_rx) = channel::<InMessage>();

        // create channel from module to broker
        let (out_tx,out_rx) = channel::<OutMessage>();

        // because of move semantics
        let local_name = name.to_string();

        // start thread, pass incoming messages to the handler
        thread::spawn(move || {

            println!("started thread for module '{}'",local_name);

            // blocking wait for a message
            while let Ok(in_message) = in_rx.recv() {

                // and execute it
                handler(in_message,&out_tx);
            }
        });

        // return the running module to the broker
        Module {
            name: name.to_string(),
            subscriptions: RefCell::new(HashSet::new()),
            in_tx: in_tx,
            out_rx: out_rx,
        }    
    }
}

// A few test module handlers:

// The camera module reads camera hardware (here triggered by a timer)
fn camera_handler(in_message: InMessage,out_tx: &Sender<OutMessage>) {

    match in_message {

        // camera does startup, initialize hardware, etc.
        InMessage::Startup => {
            println!("camera: Startup");
        },

        // camera does shutdown
        InMessage::Shutdown => {
            println!("camera: Shutdown");
        },

        // camera receives a timer tick
        InMessage::Tick => {
            println!("camera: Tick");
            println!("camera: Reading the camera hardware and publishing the video frame.");
            out_tx.send(
                OutMessage::Generic("/frames".to_string(),AppMessage::VideoFrame)
            ).expect("out_tx.send() failed!");
        },

        // everything else camera doesn't care about
        _ => { },
    }
}

fn face_detector_handler(in_message: InMessage,out_tx: &Sender<OutMessage>) {
    
    match in_message {

        // face detector does startup, subscribe to the video topic
        InMessage::Startup => {
            println!("face_detector: Startup");
            println!("face_detector: Subscribing to '/frames'");
            out_tx.send(
                OutMessage::Subscribe("/frames".to_string())
            ).expect("face_detector: failed to subscribe to '/frames'.");
        },

        // face detector does shutdown
        InMessage::Shutdown => {
            println!("face_detector: Shutdown");
        },

        // face detector wants to know when a subscription starts
        InMessage::Subscribed(topic) => {
            println!("face_detector: Broker notified me that I'm now subscribed to '{}'",topic);
        },

        // face detector wants to know when a subscription ends
        InMessage::Unsubscribed(topic) => {
            println!("face_detector: Broker notified me that I've unsubscribed from '{}'",topic);
        },

        // face detector gets an AppMessage
        InMessage::Generic(topic,app_message) => {

            match app_message {

                // face detector receives video frame
                AppMessage::VideoFrame => {
                    println!("face_detector: Received video frame from '{}'",topic);
                    println!("face_detector: Detecting face");
                    out_tx.send(
                        OutMessage::Generic("/faces".to_string(),AppMessage::VideoFrame)
                    ).expect("face_detector: failed to send video frame!");
                },

                // and nothing else
                _ => { },
            }
        },

        // and nothing else
        _ => { },
    }
}

fn display_handler(in_message: InMessage,out_tx: &Sender<OutMessage>) {
    
    match in_message {

        // display does startup, subscribes to face coordinate topic
        InMessage::Startup => {
            println!("display: Startup");
            println!("display: Subscribing to '/faces'");
            out_tx.send(
                OutMessage::Subscribe("/faces".to_string())
            ).expect("display: failed to subscribe to '/faces'");
        },

        // display does shutdown
        InMessage::Shutdown => {
            println!("display: Shutdown");
        },

        // display wants to know when a subscription starts
        InMessage::Subscribed(topic) => {
            println!("display: Broker notified me that I'm now subscribed to '{}'",topic);
        },

        // display wants to know when a subscription ends
        InMessage::Unsubscribed(topic) => {
            println!("display: Broker notified me that I've unsubscribed from '{}'",topic);
        },

        // display gets AppMessage
        InMessage::Generic(topic,app_message) => {

            match app_message {

                // new face coordinates
                AppMessage::FaceCoords => {
                    println!("display: Received face coordinates from '{}'",topic);
                },

                // and nothing else
                _ => { },
            }
        },

        // and nothing else
        _ => { },
    }
}

// The main broker thread
fn main() {
    
    // all currently running modules
    let mut modules: Vec<Module> = Vec::new();

    // start the modules
    modules.push(Module::new("Camera",camera_handler));
    modules.push(Module::new("FaceDetector",face_detector_handler));
    modules.push(Module::new("Display",display_handler));

    // and why not another one
    modules.push(Module::new("OtherDisplay",display_handler));

    // endless loop
    loop {

        // This logic needs to improve. There should be a timer here also...
        let mut did_something = false;

        // for each module
        for module in &modules {

            // flush incoming messages, if any
            while let Ok(out_message) = module.out_rx.try_recv() {

                did_something = true;

                match out_message {

                    // module wants to subscribe to a topic
                    OutMessage::Subscribe(topic) => {
                        module.subscriptions.borrow_mut().insert(topic);
                    },

                    // module wants to unsubscribe from a topic
                    OutMessage::Unsubscribe(topic) => {
                        module.subscriptions.borrow_mut().remove(&topic);
                    },

                    // module sends message to a topic
                    OutMessage::Generic(topic,app_message) => {
                        for target in &modules {
                            if target.subscriptions.borrow_mut().contains(&topic) {
                                target.in_tx.send(
                                    InMessage::Generic(topic.clone(),app_message.clone())
                                ).expect(&format!("broker: failed to send AppMessage to module '{}'",target.name));
                            }
                        }
                    },
                }
            }
        }

        // if no messages were passed, add a tiny wait here
        if !did_something {
            thread::sleep(Duration::from_millis(100));
        }
    }
}
