// TRY 1: Basic Pub/Sub Messaging Kernel Example
// Desmond Germans, Ph.D; www.germansmedia.nl

// This is rougly based on ROS Pub/Sub, after what was explored earlier.

// Notes:
// - Exclusively reactive modules based on message handlers alone might not be the way forward, there is also need for active loops and checks.
// - This implementation currently still misses a timer tick.
// - The M in MPSC is not particularly useful, since the broker might also want to send messages to a single module.

use {
    std::{
        thread,
        sync::mpsc::{
            Sender,
            channel,
        },
        collections::{
            HashSet,
            HashMap,
        },
        cell::RefCell,
    }
};

// Module ID
type MID = u64;

// The generic handler type
type Handler = fn(MID,Message,&Sender<Message>);

// Application-specific messages
#[derive(Clone)]
pub enum AppMessage {
    VideoFrame,               // hypothetical video frame
    FaceCoords,               // hypothetical face coordinates
}

// Overall messaging structure
#[derive(Clone)]
pub enum Message {
    Startup,                  // initialize the module
    Shutdown,                 // shut down the module
    Tick,                     // timer tick signal (just for this example)
    Subscribe(MID,String),    // "subscribe me to a topic"
    Unsubscribe(MID,String),  // "unsubscribe me from a topic"
    App(String,AppMessage),   // application-specific message to/from a topic
}

// A module
pub struct Module {
    name: String,                             // name of the module
    mid: MID,                                 // unique module ID
    subscriptions: RefCell<HashSet<String>>,  // topics this module receiving from
    in_tx: Sender<Message>,                   // broker-side sender to this module
}

// The broker owns the modules, so this API is from the broker's point of view
impl Module {
    fn new(name: &str,mid: MID,out_tx: Sender<Message>,handler: Handler) -> Module {

        println!("Creating module '{}'",name);

        // create channel from broker to module
        let (in_tx,in_rx) = channel::<Message>();

        // because of move semantics
        let local_name = name.to_string();

        // start thread, pass incoming messages to the handler
        thread::spawn(move || {

            println!("started thread for module '{}'",local_name);

            // blocking wait for a message
            while let Ok(message) = in_rx.recv() {

                // and execute it
                handler(mid,message,&out_tx);
            }
        });

        // return the running module to the broker
        Module {
            name: name.to_string(),
            mid: mid,
            subscriptions: RefCell::new(HashSet::new()),
            in_tx: in_tx,
        }    
    }
}

// A few test module handlers:

// The camera module reads camera hardware (here triggered by a timer)
fn camera_handler(_mid: MID,message: Message,out_tx: &Sender<Message>) {

    match message {

        // camera does startup, initialize hardware, etc.
        Message::Startup => {
            println!("camera: Startup");
        },

        // camera does shutdown
        Message::Shutdown => {
            println!("camera: Shutdown");
        },

        // camera receives a timer tick
        Message::Tick => {
            println!("camera: Tick");
            println!("camera: Reading the camera hardware and publishing the video frame.");
            out_tx.send(
                Message::App("/frames".to_string(),AppMessage::VideoFrame)
            ).expect("out_tx.send() failed!");
        },

        // everything else camera doesn't care about
        _ => { },
    }
}

fn face_detector_handler(mid: MID,message: Message,out_tx: &Sender<Message>) {
    
    match message {

        // face detector does startup, subscribe to the video topic
        Message::Startup => {
            println!("face_detector: Startup");
            println!("face_detector: Subscribing to topic '/frames'");
            out_tx.send(
                Message::Subscribe(mid,"/frames".to_string())
            ).expect("face_detector: failed to subscribe to topic '/frames'.");
        },

        // face detector does shutdown
        Message::Shutdown => {
            println!("face_detector: Shutdown");
        },

        // face detector gets an AppMessage
        Message::App(topic,app_message) => {

            match app_message {

                // face detector receives video frame
                AppMessage::VideoFrame => {
                    println!("face_detector: Received video frame from topic '{}'",topic);
                    println!("face_detector: Detecting face");
                    out_tx.send(
                        Message::App("/faces".to_string(),AppMessage::FaceCoords)
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

fn display_handler(mid: MID,message: Message,out_tx: &Sender<Message>) {
    
    match message {

        // display does startup, subscribes to face coordinate topic
        Message::Startup => {
            println!("display: Startup");
            println!("display: Subscribing to topic '/faces'");
            out_tx.send(
                Message::Subscribe(mid,"/faces".to_string())
            ).expect("display: failed to subscribe to topic '/faces'");
        },

        // display does shutdown
        Message::Shutdown => {
            println!("display: Shutdown");
        },

        // display gets AppMessage
        Message::App(topic,app_message) => {

            match app_message {

                // new face coordinates
                AppMessage::FaceCoords => {
                    println!("display: Received face coordinates from topic '{}'",topic);
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
    
    // create channel from all modules to broker
    let (out_tx,out_rx) = channel::<Message>();


    // all currently running modules
    let mut modules = HashMap::<MID,Module>::new();

    // start the modules
    modules.insert(0,Module::new("Camera",0,out_tx.clone(),camera_handler));
    modules.insert(1,Module::new("FaceDetector",1,out_tx.clone(),face_detector_handler));
    modules.insert(2,Module::new("Display",2,out_tx.clone(),display_handler));

    // and why not another one
    modules.insert(3,Module::new("OtherDisplay",3,out_tx.clone(),display_handler));

    // send startup to all modules
    for module in &modules {
        module.1.in_tx.send(Message::Startup).expect(&format!("broker: failed to send Startup to module {} ('{}')",module.1.mid,module.1.name));
    }

    // still missing: timer

    // flush incoming messages
    while let Ok(message) = out_rx.recv() {

        match message {

            // module wants to subscribe to a topic
            Message::Subscribe(mid,topic) => {
                println!("broker: subscribing module {} ('{}') to topic '{}'",mid,modules[&mid].name,topic);
                modules[&mid].subscriptions.borrow_mut().insert(topic);
            },

            // module wants to unsubscribe from a topic
            Message::Unsubscribe(mid,topic) => {
                println!("broker: unsubscribing module {} ('{}') from topic '{}'",mid,modules[&mid].name,topic);
                modules[&mid].subscriptions.borrow_mut().remove(&topic);
            },

            // module sends message to a topic
            Message::App(topic,app_message) => {
                for target in &modules {
                    if target.1.subscriptions.borrow_mut().contains(&topic) {
                        target.1.in_tx.send(
                            Message::App(topic.clone(),app_message.clone())
                        ).expect(&format!("broker: failed to send AppMessage to module {} ('{}')",target.1.mid,target.1.name));
                    }
                }
            },

            // discard anything else
            _ => { },
        }
    }
}
