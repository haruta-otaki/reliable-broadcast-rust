use core::panic;
use std::{vec, fmt::Debug, hash::Hash, collections::HashMap, marker::PhantomData};
use serde::{Serialize, Deserialize, de::DeserializeOwned};

use tokio::{task::JoinHandle, sync::mpsc::{self, Receiver, Sender}};
use futures::future::join_all;
use async_trait::async_trait; 

use crate::{aggregated_witness::AggregatedReport, barycentric_agreement::BarycentricReport, basic::{BasicCommunication, BasicQueues, Message, MessageChannels, RecvObject}}; 
use crate::witness::{Report, ReportChannels};
use crate::json::{JsonConversion};



// # Trait Description:
// This trait extends `BasicCommunication` to support a reliable broadcast protocol. 
// It enables a thread to participate in multi-instance consensus by handling signals: Input, Echo, and Vote.
// # Inherits:
// * BasicCommunication - A trait that provides ID, local queue, and base channel access.
#[async_trait]
pub trait ReliableCommunication<T>: BasicCommunication<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
{
    // # Method Description:
    // This method initiates a reliable broadcast by wrapping a message with protocol metadata
    // and broadcasting it to all participants via signal channels.
    //
    // # Parameters:
    // * message - The content of the message to broadcast as a `String`.
    // * instance_number - The consensus instance number associated with this broadcast.
    // * round_number - The round number within the consensus instance.
    //
    // # Returns:
    // * A future that asynchronously broadcasts the signal to all registered signal receivers.
    fn reliable_broadcast(&mut self, message: T, instance_number: u32, round_number: u32) -> impl Future<Output = ()>  {
        let protocol_information = String::from("reliable");
        let sent_message = Message::new(protocol_information, *self.get_id(), message, None, Some(instance_number), round_number);
        let input = Signal::new(SignalType::Input, ObjectContent::Message(sent_message), instance_number, round_number);
        self.get_signal_channels().broadcast_signal(input)
    }

    // # Method Description:
    // This method retrieves a reliably delivered message from the local queue, blocking
    // until a valid message matching the specified instance and round is available.
    //
    // # Parameters:
    // * id - Optional `u32` representing a specific sender's thread ID. If provided,
    //        the method will only retrieve from that sender’s queue.
    // * instance_number - The consensus instance number associated with the message.
    // * round_number - The round number within the consensus instance.
    //
    // # Returns:
    // * A `Message` instance retrieved from the queue.
    // # Panics:
    // * If the retrieved object is a `Collection` instead of a `Message`.
    async fn reliable_recv(&mut self, id: Option<u32>, instance_number: u32, round_number: u32) -> Message<T> {
        let protocol_information = String::from("reliable");
        match 
        self.get_queues().basic_recv(id, protocol_information, Some(instance_number), round_number).await {
            RecvObject::Message(message) => {
                return message
            },
            RecvObject::Collection(_) => {panic!("Error: retreived Vec<Message> instead of Message")},
        }
    }
 
    fn initialize_reliable_handle(&mut self) -> JoinHandle<()>;

    // # Method Description:
    // This method terminates the asynchronous thread associated with the thread's reliable broadcast mechanics. 
    //
    // # Parameters:
    // * reliable_handle - A `JoinHandle<()>` representing the spawned handle responsible for the designated thread's reliable broadcast mechanics.
    fn terminate_reliable_handle(&self, reliable_handle: JoinHandle<()>) {
        println!("id: {}, terminating reliable_handle...", self.get_id());
        reliable_handle.abort();
    }

    // # Method Description:
    // This function constructs a unique string identifier for a signal instance by combining 
    // protocol metadata, sender ID, content type, instance number, and round number. 
    // It ensures differentiation between the messages, reports, and aggregated reports 
    // recorded across consensus rounds and instances.
    //
    // # Parameters:
    // * signal - A `Signal` instance containing protocol metadata and content.
    //
    // # Returns:
    // * A String identifier in the format: "<protocol>::<sender_id>::<content_type>::<instance_number>::<round_number>"
    fn get_instance_id(thread_id:u32, signal: Signal<T>) -> String {
        let instance_number = signal.get_instance_number();
        let round_number = signal.get_round_number();
        match signal.get_content() {
            ObjectContent::Message(message) => {
                return format!("{}::{}::{}::{}::{}::{}", 
                thread_id, message.get_protocol_information(), message.get_id(), "message", instance_number, round_number);
            },
            ObjectContent::Report(report) => {
                return format!("{}::{}::{}::{}::{}::{}", 
                thread_id, report.get_protocol_information(), report.get_id(), "report", instance_number, round_number);
            },
            ObjectContent::AggregatedReport(aggregated_report) => {
                return format!("{}::{}::{}::{}::{}::{}", 
                thread_id, aggregated_report.get_protocol_information(), aggregated_report.get_id(), "aggregated report", instance_number, round_number);
            },
            ObjectContent::BarycentricReport(barycentric_report) => {
                return format!("{}::{}::{}::{}::{}::{}", 
                thread_id, barycentric_report.get_protocol_information(), barycentric_report.get_id(), "barycentric report", instance_number, round_number);
            },
        }
    }

    async fn upon_input(thread_id: u32, thread_signal_channel: &SignalChannels<T>, signal: Signal<T>);
    async fn upon_echo(thread_id: u32, thread_signal_channel: &SignalChannels<T>, signal: Signal<T>);
    async fn upon_vote(thread_id: u32, thread_channel: ChannelType<T>, signal: Signal<T>); 
    
    fn get_signal_channels(&self) -> &SignalChannels<T>;
    fn take_reliable_handle_rx(&mut self) -> Receiver<String>;
}

// # Struct Description:
// This struct manages a collection of ReliableCommunicator instances to enable reliable broadcast communication
// among asynchronous threads. Each communicator is initialized with both standard and signal-based communication
// channels to support protocols like reliable broadcast.
//
// # Fields:
// * reliable_communicators - A vector of ReliableCommunicator instances.
pub struct ReliableHub<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    reliable_communicators: Vec<ReliableCommunicator<T>>
}
 
impl<T> ReliableHub<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    pub fn new(transmitters: Vec<Sender<String>>, mut receivers: Vec<Receiver<String>>, thread_count: u32) -> Self {  
        let mut reliable_communicators = vec![];
        let mut handle_transmitters = vec![];
        let mut handle_receivers = vec![];

        for _ in 0..(thread_count) {
            let (handle_tx, handle_rx) = mpsc::channel(256); 
            handle_transmitters.push(handle_tx);
            handle_receivers.push(handle_rx);
        }
        
        for i in 0..(thread_count) {
            let handle_rx = handle_receivers.remove(0);
            let rx = receivers.remove(0);
            reliable_communicators.push(ReliableCommunicator::new(transmitters.clone(), rx, thread_count, i as u32, handle_transmitters.clone(), handle_rx));
        }
        
        Self {
            reliable_communicators
        }
    }
 
    // # Method Description:
    // This method removes and returns the next available `ReliableCommunicator` from the hub.
    // # Returns:
    // * A ReliableCommunicator instance.
    pub fn create_reliable_communicator(&mut self) -> ReliableCommunicator<T>{
        self.reliable_communicators.remove(0)
    }
 }

 
// # Struct Description:
// The `ReliableCommunicator` extends basic thread communication by introducing reliable broadcast 
// functionality. It manages both standard message channels and specialized signal channels for 
// protocol-level coordination.
//
// # Fields:
// * id - The unique identifier of the thread.
// * basic_channels - A `MessageChannels` instance that handles standard inter-thread communication.
// * signal_channels - A `SignalChannels` instance that handles protocol-specific signal broadcasting.
// * queues - A `BasicQueues` instance that stores incoming messages for this thread.
// * handle_rx - An receiver for signal-related messages, used by the async task that 
//               processes protocol-level coordination messages.
pub struct ReliableCommunicator<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    id: u32, 
    basic_channels: MessageChannels<T>, 
    signal_channels: SignalChannels<T>, 
    queues: BasicQueues<T>,
    handle_rx: Option<Receiver<String>>, 
}

impl<T> ReliableCommunicator<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    fn new(transmitters: Vec<Sender<String>>, receiver: Receiver<String>, thread_count: u32, id: u32, handle_transmitters: Vec<Sender<String>>, handle_rx: Receiver<String>) -> Self {
        let basic_channels = MessageChannels::new(transmitters.clone());
        let signal_channels = SignalChannels::<T>::new(handle_transmitters.clone());
        let queues = BasicQueues::new(receiver, thread_count);
        let handle_rx = Some(handle_rx);

        Self {
            id, 
            basic_channels,
            signal_channels,
            queues,
            handle_rx, 
        }
    }
}

#[async_trait]
impl<T> ReliableCommunication<T> for ReliableCommunicator<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
{
    // # Method Description:
    // Spawns an asynchronous background task that listens for incoming signal messages 
    // (Input, Echo, Vote).
    // The task tracks instance states, applies threshold-based transitions, 
    // and ensures messages are delivered once protocol conditions are met.
    //
    // # Returns:
    // * A `JoinHandle` to the spawned task, that runs until explicitly terminated.
    fn initialize_reliable_handle(&mut self) -> JoinHandle<()>{
        println!("initializing reliable handle...");

        let thread_id = *self.get_id(); 
        let thread_channel = self.get_channels().clone(); 
        let thread_signal_channel = self.get_signal_channels().clone();
        let thread_count = thread_channel.get_channels().len() as u32; 
        let mut receiver = self.take_reliable_handle_rx(); 

        let faulty_threads = (thread_count - 1) / 3;
        let validity_threshold = thread_count - faulty_threads + 1;
        let agreement_threshold = faulty_threads + 1;
        let mut reliable_broadcast_monitor: HashMap<String, ReliableInstanceMonitor> = HashMap::new();

        
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(received_signal) = receiver.recv() => {
                        let signal = match Signal::read_json(&received_signal) {
                            Ok(correct_signal) => correct_signal,
                            Err(_)=> { continue },
                        };

                        let instance_id = Self::get_instance_id(thread_id, signal.clone()); 
                        if let SignalType::Input = signal.get_signal() {
                            match reliable_broadcast_monitor.get(&instance_id) {
                                Some(_) => {
                                    panic!("Error: instance id already used")
                                },
                                None => {
                                    reliable_broadcast_monitor.insert(instance_id.clone(), ReliableInstanceMonitor::new());
                                },
                            }
                        }

                        let instance = reliable_broadcast_monitor.get_mut(&instance_id).unwrap(); 
                        let state = &mut instance.state; 
                        let count = &mut instance.count; 

                        match signal.get_signal()
                        {
                            SignalType::Input => {
                                if state.echo == false {
                                    Self::upon_input(thread_id, &thread_signal_channel, signal).await;
                                    state.echo = true;
                                } else { continue }
                            },
                            SignalType::Echo => {
                                count.echo += 1;

                                if count.echo >= validity_threshold && state.vote == false{
                                    Self::upon_echo(thread_id, &thread_signal_channel, signal).await;
                                    state.vote = true;
                                } else if count.echo >= agreement_threshold && state.echo == false {
                                    Self::upon_input(thread_id, &thread_signal_channel, signal).await;
                                    state.echo = true;
                                } else { continue }
                            },
                            SignalType::Vote => {
                                count.vote += 1;
    
                                if count.vote >= validity_threshold && state.deliver == false {
                                    let channel = ChannelType::MessageChannels(thread_channel.clone());
                                    Self::upon_vote(thread_id, channel, signal).await;
                                    state.deliver = true;
                                } else if count.vote >= agreement_threshold && state.vote == false {
                                    Self::upon_echo(thread_id, &thread_signal_channel, signal).await;
                                    state.vote = true;
                                } else { continue }
                            }
                        }
                    }
                }
            }
        });
        handle
    }

    // # Method Description:
    // As the first acknowledgment step in the reliable broadcast protocol,
    // handles an `Input` signal by wrapping and broadcasting the original content as an `Echo` signal to all participants.
    //
    // # Parameters:
    // * thread_id - The ID of the current thread processing the signal.
    // * thread_signal_channel - The channel used to broadcast the `Echo` signal.
    // * signal - The received `Input` signal.

    async fn upon_input(thread_id: u32, thread_signal_channel: &SignalChannels<T>, signal: Signal<T>){
        println!("id {}, instance: {}, echoing...", thread_id, signal.get_instance_number());

        let echo = Signal::new(SignalType::Echo, signal.get_content().clone(), signal.get_instance_number(), signal.get_round_number());
        thread_signal_channel.broadcast_signal(echo).await;
    }

    // # Method Description:
    // As the agreement step in the reliable broadcast protocol,
    // handles an `Echo` signal by broadcasting a `Vote` signal once the threshold is reached by the same process used to create the `Echo` signal.
    //
    // # Parameters:
    // * thread_id - The ID of the current thread processing the signal.
    // * thread_signal_channel - The channel used to broadcast the `Vote` signal.
    // * signal - The received `Echo` signal.
    async fn upon_echo(thread_id: u32, thread_signal_channel: &SignalChannels<T>, signal: Signal<T>) {
        println!("id {}, instance: {}, voting...", thread_id, signal.get_instance_number());

        let vote = Signal::new(SignalType::Vote, signal.get_content().clone(), signal.get_instance_number(), signal.get_round_number());
        thread_signal_channel.broadcast_signal(vote).await; 
    }
 

    // # Method Description:
    // As the completion step in the reliable broadcast protocol,
    // handles a `Vote` signal by delivering the final message to the application layer via `MessageChannels`.
    // Panics if the channel or content type does not match expectations.
    //
    // # Parameters:
    // * thread_id - The ID of the current thread processing the signal.
    // * channel - The channel used to deliver the final message (`MessageChannels` expected).
    // * signal - The received `Vote` signal.
    async fn upon_vote(thread_id: u32, channel: ChannelType<T>, signal: Signal<T>)  {
        println!("id {}, instance: {}, delivering...",thread_id,  signal.get_instance_number());
        let object = signal.get_content().clone(); 
        
        if let (ChannelType::MessageChannels(thread_channel), ObjectContent::Message(message)) = (channel, object) {
            thread_channel.send_message(thread_id, message).await;
        } else {
            panic!("Error: received incompatible channel or object type for reliable broadcast");
        }
    }
    
    fn get_signal_channels(&self) -> &SignalChannels<T> {
        &self.signal_channels
    }

    fn take_reliable_handle_rx(&mut self) -> Receiver<String> {
        self.handle_rx.take().unwrap()
    }
}

impl<T> BasicCommunication<T> for ReliableCommunicator<T>
where 
    T: Debug + Clone + Serialize + DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
{
    fn get_channels(&self) -> &MessageChannels<T> {
        &self.basic_channels
    }

    fn get_queues(&mut self) -> &mut BasicQueues<T> {
        &mut self.queues
    }

    fn get_id(& self) -> &u32 {
        &self.id
    }
}


// # Struct Description:
// This struct manages a collection of channel transmitters used to broadcast serialized `Signal` messages.
// It enables reliable and parallel signal transmission to multiple asynchronous threads.
// # Fields:
// * handle_transmitters - A vector of senders used to send serialized signal messages to each thread.
#[derive(Clone)]
pub struct SignalChannels<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    handle_transmitters: Vec<Sender<String>>,
    _marker: PhantomData<T>,
}

impl<T> SignalChannels<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    // # Method Description:
    // Asynchronously broadcasts a given Signal to all threads by serializing it into a JSON string
    // and sending it through all registered transmitters.
    // # Parameters:
    // * signal - The Signal to broadcast to all receivers.
    pub(crate) fn broadcast_signal(&self, signal: Signal<T>) -> impl Future<Output = ()> {
        let mut send_fns= vec![];
        for handle_tx in self.get_handle_channels() {
            let new_signal = signal.clone(); 
            send_fns.push(handle_tx.send(new_signal.write_json()));
        }; 
        async move {
            join_all(send_fns).await; 
        }
    }  

    pub fn get_handle_channels(&self) -> &Vec<Sender<String>> {
        &self.handle_transmitters
    }

    pub fn new(handle_transmitters: Vec<Sender<String>>) -> Self {
        Self {
            handle_transmitters,
            _marker: PhantomData,
        }
    }

}

// # Enum Description:
// This enum represents the type of channel through which messages or reports are delivered
// in the communication framework.
//
// # Variants:
// * MessageChannels - A channel used for sending and receiving standard messages between threads.
// * ReportChannels - A channel used for sending and receiving reports, from witness or aggregated witness communication protocols.
pub enum ChannelType<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    MessageChannels(MessageChannels<T>), 
    ReportChannels(ReportChannels<T>),
}

// # Enum Description:
// This enum represents the type of signal exchanged between threads as part of the reliable broadcast protocol.
// # Variants:
// * Input - The initial signal sent by the origin thread.
// * Echo - The signal echoed by threads to confirm receipt.
// * Vote - The final decision signal cast by threads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalType {
    Input, 
    Echo,
    Vote,
}

// # Enum Description:
// This enum represents the content of a signal exchanged between threads in the communication framework.
// It is used to encapsulate different types of payloads, including standard messages, individual reports, 
// or aggregated reports for protocol-level operations.
//
// # Variants:
// * Message - A standard message sent between threads.
// * Report - A collection of messages represented as a report generated by a thread.
// * AggregatedReport - A collection of reports combined into a single aggregated report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObjectContent<T>{
    Message(Message<T>), 
    Report(Report<T>),
    AggregatedReport(AggregatedReport<T>),
    BarycentricReport(BarycentricReport<T>)
}

impl<T> ObjectContent<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    pub fn get_round_number(&self) -> u32{
        match self {
            ObjectContent::Message(message) => message.get_round_number(),
            ObjectContent::Report(report) => report.get_round_number(),
            ObjectContent::AggregatedReport(aggregated_report) => aggregated_report.get_round_number(),
            ObjectContent::BarycentricReport(barycentric_report) => barycentric_report.get_round_number(),
        }
    }

    pub fn get_protocol_information(&self) -> &String{
        match self {
            ObjectContent::Message(message) => message.get_protocol_information(),
            ObjectContent::Report(report) => report.get_protocol_information(),
            ObjectContent::AggregatedReport(aggregated_report) => aggregated_report.get_protocol_information(),
            ObjectContent::BarycentricReport(barycentric_report) => barycentric_report.get_protocol_information(),
        }
    } 
}

// # Struct Description: 
// This struct wraps a signal type and its associated content, representing a protocol-level signal
// exchanged between threads to coordinate reliable communication. It includes both instance and round
// numbers to track the progress of multi-round consensus protocols.
//
// # Fields: 
// * signal - The type of signal indicating the stage of the protocol.
// * content - The payload of the signal.
// * instance_number - The identifier of the consensus instance.
// * round_number - The round number associated with this signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal<T> {
    signal: SignalType,
    content: ObjectContent<T>, 
    instance_number: u32,
    round_number: u32
}

impl<T> Signal<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    pub fn get_signal(&self) -> &SignalType {
        &self.signal
    }

    pub fn get_content(&self) -> &ObjectContent<T> {
        &self.content
    }

    pub fn get_instance_number(&self) -> u32 {
        self.instance_number
    }

    pub fn get_round_number(&self) -> u32 {
        self.round_number
    }

    pub fn new(signal: SignalType, content: ObjectContent<T>, instance_number: u32, round_number: u32) -> Self {
        Self {
            signal,
            content,
            instance_number,
            round_number
        }
    }
}

impl<T> JsonConversion<Signal<T>> for Signal<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{}

// # Struct Description:
// This struct tracks the progress of a single consensus instance in the reliable broadcast protocol.
//
// # Fields:
// * state - A `ReliableInstanceState` struct representing whether echo, vote, or delivery has occurred.
// * count - A `ReliableInstanceCount` struct counting the number of Echo and Vote signals received.
pub struct ReliableInstanceMonitor {
    pub state: ReliableInstanceState,
    pub count: ReliableInstanceCount, 
}

impl ReliableInstanceMonitor {
    pub fn new() -> Self {
        let state = ReliableInstanceState::new();
        let count = ReliableInstanceCount::new();
        Self {
            state,
            count
        }
    }
}

// # Struct Description:
// This struct counts the number of signals received in a single consensus instance.
//
// # Fields:
// * echo - The number of Echo signals received for this instance.
// * vote - The number of Vote signals received for this instance.
pub struct ReliableInstanceCount {
    pub echo: u32,
    pub vote: u32,
}

impl ReliableInstanceCount {
    pub fn new() -> Self {
        let echo = 0; 
        let vote = 0; 
        Self {
            echo,
            vote
        }
    }
}


// # Struct Description:
// This struct represents the boolean state of a single consensus instance in reliable broadcast.
//
// # Fields:
// * echo - Boolean state of whether the Input signal has been echoed by this thread.
// * vote - Boolean state of whether the Echo signals have triggered a vote by this thread.
// * deliver - Boolean state of whether the message has been delivered by this thread.
pub struct ReliableInstanceState {
    pub echo: bool,
    pub vote: bool,
    pub deliver: bool,
}

impl ReliableInstanceState {
    pub fn new() -> Self {
        let echo = false; 
        let vote = false; 
        let deliver = false; 
        Self {
            echo,
            vote,
            deliver
        }
    }
}