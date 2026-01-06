use core::panic;
use std::{vec, fmt::Debug, hash::Hash, marker::PhantomData};
use tokio::sync::mpsc::{Receiver, Sender};
use std::collections::{HashMap, VecDeque};
use futures::future::join_all;
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use async_trait::async_trait; 

use crate::json::{JsonConversion};
use crate::witness::Report;

// # Trait Description:
// A trait that defines basic communication behavior for a node in a distributed system:
// send messages to specific nodes, broadcast messages to all nodes, and receive messages from a local queue
#[async_trait]
pub trait BasicCommunication<T> 
where 
    T: Debug + Clone + Serialize + DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
{
    fn get_channels(&self) -> &MessageChannels<T>;
    fn get_queues(&mut self) -> &mut BasicQueues<T>;
    fn get_id(& self) -> &u32;

    // # Method Description:
    // This method sends a message to a specific node by ID.
    // # Parameters
    // * `id` - The recipient's node ID.
    // * `message` - The message content to send.
    // * `round_number` - The current communication round, to track consensus or protocol progress.
    // # Returns
    // A future that sends the message and resolves when the send operation completes.
    fn basic_send(&mut self, id: u32, message: T, round_number: u32) -> impl Future<Output = ()> {
        let protocol_information = String::from("basic") ;
        let sent_message = Message::new(protocol_information ,*self.get_id(), message, None, None, round_number); 
        self.get_channels().send_message(id, sent_message)
    }

    // # Method Description:
    // This method broadcasts a message to all other nodes.
    // # Parameters
    // * `message` - The message content to broadcast.
    // * `round_number` - The current communication round, to track consensus or protocol progress.
    // # Returns
    // A future that broadcasts the message to all peers and resolves when all sends complete.
    fn basic_broadcast(&mut self, message: T, round_number: u32) -> impl Future<Output = ()> {
        let protocol_information = String::from("basic") ;
        let sent_message = Message::new(protocol_information, *self.get_id(), message, None, None, round_number);
        self.get_channels().broadcast_message(sent_message)
    }

    // # Method Description:
    // This method receives the next available message from the local queue.
    // # Parameters
    // * `id` - Optional ID of the sender to filter by; if `None`, receives any message.
    // * `round_number` - The current communication round, to track consensus or protocol progress.
    // # Returns
    // A `Message` instance received from the local queue, once available.
    async fn basic_recv(&mut self, id: Option<u32>, round_number: u32) -> Message<T> {
        let protocol_information = String::from("basic") ;
        match
        self.get_queues().basic_recv(id, protocol_information, None, round_number).await {
            RecvObject::Message(message) => {                       
                return message
            },
            RecvObject::Collection(_) => {panic!("Error: retreived Vec<Message> instead of Message")},
        }
    }
}


// # Struct Description:
// This struct manages multiple BasicCommunicator instances to support message passing
// between asynchronous threads. It assigns each communicator a receiver and shares a
// common set of transmitters for inter-thread communication.

// # Fields:
// * basic_communicators - A vector of BasicCommunicator instances initialized for each thread
pub struct BasicHub<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    basic_communicators: Vec<BasicCommunicator<T>>,
}

impl<T> BasicHub<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    pub fn new(transmitters: Vec<Sender<String>>, mut receivers: Vec<Receiver<String>>, thread_count: u32) -> Self {
        let mut basic_communicators = vec![];
        for i in 0..thread_count {
            let rx = receivers.remove(0); 
            basic_communicators.push(BasicCommunicator::new(transmitters.clone(), rx, thread_count, i as u32));
        }
        Self {
            basic_communicators
        }
    }

    // # Method Description:
    // This method removes and returns the first available BasicCommunicator from the hub.
    // # Returns:
    // * A BasicCommunicator instance.
    pub fn create_basic_communicator(&mut self) -> BasicCommunicator<T>{
        self.basic_communicators.remove(0)
    }
}

// # Struct Description:
// This struct supports message sending between asynchronous threads.
// It holds a list of channel transmitters to facilitate direct and broadcast communication,
// as well as queues for managing received messages.
// # Fields:
// * id - The thread’s unique ID.
// * channels - A struct encapsulating all transmitters for outgoing messages.
// * queues - A struct that handles incoming messages via the thread’s local receiver.
pub struct BasicCommunicator<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    id: u32,
    channels: MessageChannels<T>, 
    queues: BasicQueues<T>,
}

impl<T> BasicCommunicator<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    fn new(transmitters: Vec<Sender<String>>, rx: Receiver<String>, thread_count: u32, id: u32) -> Self {
        let channels = MessageChannels::<T>::new(transmitters);
        let queues = BasicQueues::new(rx, thread_count);

        Self {
            id, 
            channels,
            queues
        }
    }
}
impl<T> BasicCommunication<T> for BasicCommunicator<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned+ PartialEq + Eq + Hash + Send + Sync + 'static,
{
    fn get_channels(&self) -> &MessageChannels<T> {
        &self.channels
    }

    fn get_queues(&mut self) -> &mut BasicQueues<T> {
        &mut self.queues
    }

    fn get_id(& self) -> &u32 {
        &self.id
    }

}



#[derive(Debug, Clone)]
// # Struct Description:
// This struct supports message sending between asynchronous threads.
// It holds a list of channel transmitters to facilitate direct and broadcast communication.
// # Fields:
// * tx_vec - A vector of cloned transmitters for sending messages to a specific thread.

/*
The PhantomData<T> is included as a field in the struct as the generic parameter T 
does not appear in any actual field of the struct, but logically contains a value of type T. 
Without the field Rust warns the struct does not use T at runtime as Rust does not allow a 
generic parameter that has no physical effect on the type’s memory layout or behavior unless 
explicitly marked. Therefore the compiler must treat MessageChannels<T> as if it carries T.
*/
pub struct MessageChannels<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    tx_vec: Vec<Sender<String>>,
    _marker: PhantomData<T>,
}

impl<T> MessageChannels<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    // # Method Description:
    // Ths method sends a message to a specific thread using its ID. The message is serialized to JSON.
    // # Parameters:
    // * id - The recipient thread’s ID
    // * message - The `Message` sent to the specified thread.
    pub(crate) fn send_message(&self, id: u32, message: Message<T>) -> impl Future<Output = ()>{
        async move {
            match self.get_channels().get(id as usize) {
                Some(channel) => {
                    let _ = channel.send(message.write_json()).await;
                    println!("sent: {:?}", &message.get_message());

                },
                None => panic!("Error: failed to find channel"),
            }
        }
    }

    // # Method Description:
    // This method broadcasts a message to all threads in the system. Each message is cloned
    // and sent individually to each thread’s channel.
    // # Parameters:
    // * message - The `Message` broadcasted to all threads.
    pub(crate) fn broadcast_message(&self, message: Message<T>) -> impl Future<Output = ()> {
        let mut send_fns= vec![];
        for tx in self.get_channels() {
            let sent_message = message.clone();
            println!("broadcast: {:?}", & sent_message.get_message());
            send_fns.push(tx.send(sent_message.write_json()));
        }; 
        async move {
            join_all(send_fns).await; 
        }
    }   

    pub fn get_channels(&self) -> &Vec<Sender<String>> {
        &self.tx_vec
    }

    pub fn new(tx_vec: Vec<Sender<String>>) -> Self {
        Self {
            tx_vec,
            _marker: PhantomData,
        }
    }
}



// # Struct Description: 
// This struct manages incoming messages and internal buffering for a single thread. 
// It acts as a local message handler, receiving messages from other threads and 
// organizing them into individual queues based on sender ID.
//
// # Fields: 
// * rx - a incoming asynchronous channel for receiving raw messages
// * queues - a hashmap where each key corresponds to a sender thread's ID,
//            and each value is a queue of parsed `Message`s received from that sender.
pub struct BasicQueues<T> 
where 
    T: Debug + Clone + Serialize + DeserializeOwned + PartialEq + Eq + Hash,
{
    rx: Receiver<String>,
    queues: HashMap<u32, VecDeque<RecvObject<T>>>,
}

impl<T> BasicQueues<T>
where
    T: Debug + Clone + Serialize + DeserializeOwned + PartialEq + Eq + Hash,
{

    pub fn get_receiver(&mut self) -> &mut Receiver<String> {
        &mut self.rx
    }

    pub fn get_queues(&mut self) -> &mut HashMap<u32, VecDeque<RecvObject<T>>> {
        &mut self.queues
    }

    pub fn new(rx: Receiver<String>, thread_count: u32) -> Self {
        let mut queues: HashMap<u32, VecDeque<RecvObject<T>>> = HashMap::new(); 
        for i in 0..thread_count {
            let buffer: VecDeque<RecvObject<T>> = VecDeque::new();
            queues.insert(i, buffer);
        }

        Self {
            rx,
            queues
        }
    }
    
    // # Method Description: 
    // This method retrieves a message from the appropriate local queue. If a specific `id` is provided, 
    // it targets that sender's queue; otherwise, it searches across all queues and returns the first 
    // matching message. The function continuously checks queues until a matching message is found and 
    // blocks asynchronously until a message matching the given parameters is available. 
    //
    // # Parameters:
    // * id - Optional `u32` representing the sender's thread ID. If `None`, any available queue is searched.
    // * protocol_information - A `String` describing the protocol context.
    // * instance_number - Optional `u32` specifying the communication instance for messages received using the reliable broadcast protocol.
    // * round_number - A `u32` identifying the round of the protocol to match the correct message.
    //
    // # Returns:
    // * A `RecvObject`, that may be either:
    //   - `RecvObject::Message` containing a `Message`
    //   - `RecvObject::Collection` containing a collection of `Message`s.
    pub(crate) async fn basic_recv(&mut self, id: Option<u32>, protocol_information: String, instance_number: Option<u32>, round_number: u32) -> RecvObject<T> {
        match id {
            Some(id) => {
                loop {
                    let queue = match self.get_queues().get_mut(&id) {
                        Some(queue) => queue,
                        None => panic!("Error: queue not found"),
                    };
                    if !queue.is_empty() {
                        match Self::retreive_message(queue, &protocol_information, instance_number, round_number) {
                            Some(RecvObject::Message(message)) => {
                                println!("{} received(specified): {:?}", message.get_protocol_information(),message.get_message());                               
                                return RecvObject::Message(message)
                            },
                            Some(RecvObject::Collection(collection)) => {return RecvObject::Collection(collection)},
                            None => {},
                        };
                    } 
                    self.store_message().await;
                }
            },
            None => {
                loop {
                    for set in self.get_queues() {
                        let queue = set.1; 
                        if !queue.is_empty() {
                            match Self::retreive_message(queue, &protocol_information, instance_number, round_number) {
                                Some(RecvObject::Message(message)) => {
                                    println!("{} received(any): {:?}", message.get_protocol_information(),message.get_message());                               
                                    return RecvObject::Message(message)
                                },
                                Some(RecvObject::Collection(collection)) => {
                                    return RecvObject::Collection(collection)
                                },
                                None => {continue},
                            };
                        } 
                    }
                    self.store_message().await;
                }
            },
        }
    }

    // # Function Description:
    // This function searches a given queue for a message that matches the specified 
    // protocol information, instance number, and round number. If such a message exists, it is 
    // removed from the queue and returned; otherwise, the function returns `None`.
    //
    // # Parameters:
    // * queue - A mutable reference to a `VecDeque<RecvObject>` representing the message queue.
    // * protocol_information - A `String` describing the protocol context to match against.
    // * instance_number - Optional `u32` specifying the communication instance to filter messages.
    // * round_number - A `u32` identifying the round of the protocol.
    //
    // # Returns:
    // * `Some(RecvObject)` if a matching message is found and removed from the queue.
    // * `None` if no matching message exists in the queue.
    fn retreive_message(queue: &mut VecDeque<RecvObject<T>>, protocol_information: &String, instance_number: Option<u32>,round_number: u32) -> Option<RecvObject<T>>{
        match queue.iter().position(|object| object.get_protocol_information() == protocol_information && object.get_instance_number() == instance_number && object.get_round_number() == round_number) {
            Some(index) => return queue.remove(index),
            None => return None, 
        }
    }

    // # Method Description:
    // This asynchronous method receives a new message from the thread’s receiving channel and
    // stores it into the appropriate local queue based on the message’s sender ID.
    async fn store_message(&mut self) {
        tokio::select! {
            Some(received_message) = self.get_receiver().recv() => {
                let object: RecvObject<T>; 
                if let Ok(message) = Message::read_json(&received_message) {
                    object = RecvObject::Message(message);
                } else if let Ok(collection) = Report::read_json(&received_message) {
                    object = RecvObject::Collection(collection);
                } else {
                    return;
                }

                match self.get_queues().get_mut(& object.get_id())
                {
                    Some(queue) => {
                        match &object {
                            RecvObject::Message(message) => {
                                println!("stored: {:?}", message.get_message());                               
                            },
                            RecvObject::Collection(collection) => {
                                println!("stored: Report by id: {}", collection.get_id());
                            }
                        }
                        queue.push_back(object);
                    },
                    None => panic!("Error: failed to find buffer"), 
                }
            }
        }
    }
}

// # Enum Description:
// This enum represents the type of object that may be received from a communication queue.
// It encapsulates either a single protocol message or a collection of messages,
// enabling flexible handling of different communication outcomes.
//
// # Variants:
// * Message - Wraps a single `Message` instance received from another thread.
// * Collection - Wraps a `Report` instance, representing a collection of `Message`s.
#[derive(Debug)]
pub enum RecvObject<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned+ PartialEq + Eq + Hash,
{
    Message(Message<T>), 
    Collection(Report<T>)
}


impl<T> RecvObject<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    pub fn get_id(&self) -> u32{
        match self {
            RecvObject::Message(message) => message.get_id(),
            RecvObject::Collection(report) => report.get_id(),
        }
    }
    pub fn get_protocol_information(&self) -> &String{
        match self {
            RecvObject::Message(message) => message.get_protocol_information(),
            RecvObject::Collection(report) => report.get_protocol_information(),
        }
    }
    pub fn get_instance_number(&self) -> Option<u32>{
        match self {
            RecvObject::Message(message) => message.get_instance_number(),
            RecvObject::Collection(report) => Some(report.get_instance_number()),
        }
    }
    pub fn get_round_number(&self) -> u32{
        match self {
            RecvObject::Message(message) => message.get_round_number(),
            RecvObject::Collection(report) => report.get_round_number(),
        }
    }
}


// # Struct Description:
// This struct represents a message exchanged between threads in communication protocols.
// It stores metadata - protocol type, sender ID, instance, and round number - ensuring
// correct ordering, identification, and handling in various reliable communication protocols.
//
// # Fields:
// * protocol_information - A `String` containing the type of the executed protocol.
// * id - A `u32` representing the ID of the thread that sent the message.
// * message - A `String` containing the actual message payload.
// * instance_number - An optional `u32` identifying the instance of the protocol this message belongs to.
// * round_number - A `u32` indicating the round in which this message was sent, used for reliable broadcast or ordering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
//pub struct Message<T = String> {
pub struct Message<T> {
    protocol_information: String, 
    id: u32, 
    message: T,
    dimension: Option<u32>,
    instance_number: Option<u32>,
    round_number: u32
}

//explanation of DeserializeOwned: 
// DeserializeOwned implemented instead of Deserialize<'de> as it does not have a 
// lifetime parameter; hence, the type may be deserialized without borrowing from the input. 

// as Message<T> and other implementations of that object both have lifetime parameters in its 
// trait bounds, they clash and an error occurs. 

// as message-passing systems contain fully owned, safe objects DeserializeOwned
// interoperates cleanly with a derived Deserialize implementation
impl<T> Message<T> 
where
    T: Debug + Clone + Serialize + DeserializeOwned + PartialEq + Eq + Hash,
{
    pub fn get_protocol_information(&self) -> &String{
        &self.protocol_information
    }

    pub fn get_id(&self) -> u32{
        self.id
    }

    pub fn get_dimension(&self) -> Option<u32>{
        self.dimension.clone()
    }

    pub fn get_message(&self) -> &T {
        &self.message
    }

    pub fn get_instance_number(&self) -> Option<u32>{
        self.instance_number
    }

    pub fn get_round_number(&self) -> u32{
        self.round_number
    }



    pub fn new(protocol_information: String, id: u32, message: T, dimension: Option<u32>,instance_number: Option<u32>, round_number: u32) -> Self {
        Self {
            protocol_information, 
            id,
            message,
            dimension,
            instance_number,
            round_number
        }
    }
}

impl<T: Debug + Clone + Serialize + DeserializeOwned + PartialEq + Eq + Hash> JsonConversion<Message<T>> for Message<T> {}