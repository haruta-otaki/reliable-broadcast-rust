use core::panic;
use std::{vec, fmt::Debug, hash::Hash, collections::{HashMap}};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use tokio::{task::JoinHandle, sync::mpsc::{self, Receiver, Sender}};
use async_trait::async_trait; 

use crate:: basic::{BasicCommunication, BasicQueues, Message, MessageChannels, RecvObject}; 
use crate::reliable::{ReliableCommunication, Signal, SignalType, ChannelType, ObjectContent, SignalChannels, ReliableInstanceMonitor}; 
use crate::witness::{Report, ReportType, ReportChannels};
use crate::json::{JsonConversion};

// # Trait Description:
// This trait defines the communication behavior for threads participating in the Barycentric Agreement protocol, 
// a higher-level coordination mechanism built on top of `ReliableCommunication`. 
// It provides methods to initialize trusted messages and buddy relationships, 
// broadcast and collect Barycentric messages, generate reports, and manage async communication handles for 
// Barycentric rounds. Threads using this trait can identify consensus values, determine trusted 
// peers, and ensure agreement propagation in a multi-round consensus process.
#[async_trait]
pub trait BarycentricCommunication<T>: ReliableCommunication<T>
where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Default + Send + Sync + 'static,
{

    // # Function Description:
    // This function initializes the trusted messages for a given round based on the agreement threshold.
    // It iterates through all received Barycentric reports and compares their messages 
    // with the locally stored messages. If a message appears consistently across enough 
    // reports (meeting the `agreement_threshold`), it is marked as trusted.
    // 
    // # Parameters:
    // * thread_id - ID of the thread evaluating trust across reports.
    // * agreement_threshold - the number of occurrences required to consider a message trusted.
    // * count - a mutable reference to the `BarycentricRoundCount`, tracking per-round message counts.
    // * content - a mutable reference to the `BarycentricRoundContent` containing the reports and messages.
    //
    // # Returns:
    // * a vector of trusted `Message` objects recognized in the current round.
    fn initialize_trusted(thread_id: u32, agreement_threshold: u32, count: &mut BarycentricRoundCount, content: &mut BarycentricRoundContent<T>) -> Vec<Message<T>>{
        let mut trusted_monitor: Vec<u32> = vec![];
        let mut trusted: Vec<Message<T>> = vec![];
        let initial_message = Message::new("".to_string(), 0, T::default(), None, None, 0); 

        for _ in 0..count.messages {
            trusted_monitor.push(0);
        }

        for barycentric_report in &content.barycentric_reports {
            for report_message in barycentric_report.get_messages(){
                let id = report_message.get_id() as usize;
                if let Some(message) = content.messages.get(id) {
                    if message == report_message && message != &initial_message {
                        trusted_monitor[id] += 1; 
                    }
                }
            }
        }

        for id  in 0..trusted_monitor.len() {
            if trusted_monitor[id] >= agreement_threshold {
                 if let Some(message) = content.messages.get(id) {
                    trusted.push(message.clone());
                }
            }
        }
        if trusted.len() > 0 {
            println!("id: {}, recognize trusted values: {:?}", thread_id, trusted);
        }
        return trusted
    }

    // # Function Description:
    // This method establishes buddy relationships between threads for the current round by comparing 
    // each received Barycentric report with the local message set. A thread is considered 
    // a "buddy" if its Barycentric report matches the current thread’s messages. This helps 
    // track alignment between participants in the agreement process.
    // 
    // # Parameters:
    // * _thread_id - The ID of the current thread (not used directly in this function).
    // * messages - A mutable reference to the current thread’s vector of `Message` objects.
    // * buddies - A mutable reference to a boolean vector indicating buddy status for each peer.
    // * barycentric_reports - A mutable reference to a vector of received `BarycentricReport` objects.
    // * count - A mutable reference to the `BarycentricRoundCount` used for tracking buddies.
    fn initialize_buddies(_thread_id: u32, messages: &mut Vec<Message<T>>, buddies: &mut Vec<bool>, barycentric_reports: &mut Vec<BarycentricReport<T>>, count: &mut BarycentricRoundCount) {
        count.buddies = 0;  
        let initial_message = Message::new("".to_string(), 0, T::default(), None, None, 0); 
        let initial_report = BarycentricReport::new("".to_string(), 0, vec![initial_message.clone()], 0, 0);

        for barycentric_report in barycentric_reports {
            let id = barycentric_report.get_id() as usize;
            if messages == barycentric_report.get_messages() && barycentric_report != &initial_report {
                buddies[id] = true; 
                count.buddies += 1;  
            } else {
                buddies[id] = false; 
            }
            
        }
    }

    // # Function Description:
    // This function broadcasts a message to all threads in the Barycentric Agreement protocol. The message 
    // is wrapped into a `Signal` object with `SignalType::Input` and sent through the 
    // signal channels for asynchronous processing.
    // 
    // # Parameters:
    // * message - The message content to broadcast.
    // * value - A vector of `u32` values representing Barycentric coefficients or metadata.
    // * round_number - The current round number for message broadcasting.
    //
    // # Returns:
    // * A future resolving to `()` once the broadcast operation is complete.
    fn barycentric_agreement(&mut self, message: T, round_number: u32) -> impl Future<Output = ()> {
        let protocol_information = String::from("barycentric");
        let instance_number = 0; 
        let sent_message = Message::new(protocol_information, *self.get_id(), message, None, Some(instance_number), round_number);
        let input = Signal::new(SignalType::Input, ObjectContent::Message(sent_message), instance_number, round_number);
        self.get_signal_channels().broadcast_signal(input)
    }

    // # Function Description:
    // This method collects all messages received during the Barycentric Agreement round. It waits for 
    // a `Collection` object (a `BarycentricReport`) to be received, extracts its contained 
    // messages, and returns them for further processing.
    // 
    // # Parameters:
    // * round_number - The round number for which collection is performed.
    //
    // # Returns:
    // * A vector of `Message` objects aggregated from the collected reports.
    async fn barycentric_collect(&mut self, round_number: u32) -> Vec<Message<T>>{
        let protocol_information = String::from("barycentric");
        let thread_id = self.get_id().clone();

        match self.get_queues().basic_recv(Some(thread_id), protocol_information, Some(0), round_number).await {
            RecvObject::Message(_) => {panic!("Error: retreived Message instead of Vec<Message>")},
            RecvObject::Collection(report) => {
                println!("Agreement collected: {:?}", &report.get_messages());    
                let collection = report.get_messages().clone();
                return collection;
            },
        }
    }

    // # Function Description:
    // This function creates a `BarycentricReport` representing the current round’s state, including all 
    // locally known messages. This report is used for reliable dissemination among peers 
    // during the agreement process.
    // 
    // # Parameters:
    // * thread_id - The ID of the current thread generating the report.
    // * content - A mutable reference to the `BarycentricRoundContent` containing messages.
    // * round_number - The round number for which the report is generated.
    // * protocol_information - A string identifier for the protocol (e.g., "barycentric").
    // * count - A mutable reference to `BarycentricRoundCount` tracking messages and instances.
    //
    // # Returns:
    // * A new `BarycentricReport` object encapsulating the current round’s data.
    fn create_barycentric_report(thread_id: u32, content: &mut BarycentricRoundContent<T>, round_number: u32, protocol_information: String, count: &mut BarycentricRoundCount) -> BarycentricReport<T>{
        let protocol_information = protocol_information;
        let instance_number = count.messages; 
        BarycentricReport::new(protocol_information, thread_id, content.messages.clone(), instance_number, round_number)
    }

    // # Function Description:
    // This method terminates the currently running Barycentric handle associated with the thread. 
    // This function is typically called at the end of a communication round to abort 
    // the background task cleanly and free resources.
    // 
    // # Parameters:
    // * barycentric_handle - The asynchronous join handle for the Barycentric task being terminated.
    fn terminate_barycentric_handle(&self, barycentric_handle: JoinHandle<()>) {
        println!("id: {}, terminating barycentric_handle...", self.get_id());
        barycentric_handle.abort();
    }

    async fn reliable_broadcast_barycentric_report(thread_id: u32, thread_signal_channel: &SignalChannels<T>, content: &mut BarycentricRoundContent<T>, round_number: u32, protocol_information: String, count: &mut BarycentricRoundCount); 
    fn initialize_barycentric_handle(&mut self) -> JoinHandle<()>; 
    fn take_barycentric_handle_rx(&mut self) -> Receiver<String>;
    fn get_report_channels(&self) -> &ReportChannels<T>;

}

// # Struct Description:
// This struct manages a collection of `BarycentricCommunicator` instances, each representing a thread
// participating in the Barycentric Agreement protocol. It is responsible for initializing, storing, 
// and managing communication between all participating threads, enabling multi-round agreement and 
// consensus propagation across the system.
//
// # Fields:
// * barycentric_communicators - A vector containing all `BarycentricCommunicator` instances managed 
//   by this hub, each encapsulating the communication logic for a single participating thread.
pub struct BarycentricHub<T> 
where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
{
    barycentric_communicators: Vec<BarycentricCommunicator<T>>
}
 
impl<T> BarycentricHub<T>
where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
{
    pub fn new(transmitters: Vec<Sender<String>>, mut receivers: Vec<Receiver<String>>, thread_count: u32) -> Self {  
        let mut barycentric_communicators = vec![];
        let mut reliable_handle_transmitters = vec![];
        let mut reliable_handle_receivers = vec![];

        let mut barycentric_handle_transmitters = vec![];
        let mut barycentric_handle_receivers = vec![];

        for _ in 0..(thread_count) {
            let (reliable_handle_tx, reliable_handle_rx) = mpsc::channel(256); 
            let (barycentric_handle_tx, barycentric_handle_rx) = mpsc::channel(256); 

            reliable_handle_transmitters.push(reliable_handle_tx);
            reliable_handle_receivers.push(reliable_handle_rx);

            barycentric_handle_transmitters.push(barycentric_handle_tx);
            barycentric_handle_receivers.push(barycentric_handle_rx);
        }
        
        for i in 0..(thread_count) {
            let reliable_handle_rx = reliable_handle_receivers.remove(0);
            let barycentric_handle_rx = barycentric_handle_receivers.remove(0);
            let rx: Receiver<String> = receivers.remove(0);
            barycentric_communicators.push(BarycentricCommunicator::new(transmitters.clone(), rx, 
                thread_count, i as u32, reliable_handle_transmitters.clone(), reliable_handle_rx, barycentric_handle_transmitters.clone(), barycentric_handle_rx));
        }
        
        Self {
            barycentric_communicators
        }
    }
 
    pub fn create_barycentric_communicator(&mut self) -> BarycentricCommunicator<T>{
        self.barycentric_communicators.remove(0)
    }
 }

// # Struct Description:
// This struct represents a single thread participating in the Barycentric Agreement protocol. 
// It extends basic communication capabilities with both reliable and barycentric-specific mechanisms, 
// managing distinct communication channels for message passing, protocol signaling, and report exchange.
//
// # Fields:
// * id - The unique identifier for this thread.
// * basic_channels - Manages standard inter-thread message transmission and reception.
// * signal_channels - Handles the broadcasting and reception of protocol-level control signals 
//   (e.g., Input, Echo, Vote) for reliable and barycentric communication.
// * report_channels - Manages the transmission and reception of barycentric report objects 
//   used for consensus formation and trust evaluation.
// * queues - Stores pending messages, collections, and other data received by this thread 
//   during protocol execution.
// * reliable_handle_rx - A receiver dedicated to listening for incoming reliable broadcast signals.
// * barycentric_handle_rx - A receiver dedicated to listening for incoming barycentric broadcast signals.
pub struct BarycentricCommunicator<T>
where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
{
    id: u32, 
    basic_channels: MessageChannels<T>, 
    signal_channels: SignalChannels<T>, 
    report_channels: ReportChannels<T>,
    queues: BasicQueues<T>,
    reliable_handle_rx: Option<Receiver<String>>, 
    barycentric_handle_rx: Option<Receiver<String>>, 
}

impl<T> BarycentricCommunicator<T>
where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
{
    fn new(transmitters: Vec<Sender<String>>, receiver: Receiver<String>, 
            thread_count: u32, id: u32, reliable_handle_transmitters: Vec<Sender<String>>, reliable_handle_rx: Receiver<String>, barycentric_handle_transmitters: Vec<Sender<String>>, barycentric_handle_rx: Receiver<String>) -> Self {
        let basic_channels = MessageChannels::new(transmitters.clone());
        let signal_channels = SignalChannels::new(reliable_handle_transmitters.clone());
        let report_channels = ReportChannels::new(barycentric_handle_transmitters.clone());
        let queues = BasicQueues::new(receiver, thread_count);
        let reliable_handle_rx = Some(reliable_handle_rx);
        let barycentric_handle_rx = Some(barycentric_handle_rx);

        Self {
            id, 
            basic_channels,
            signal_channels,
            report_channels,
            queues,
            reliable_handle_rx,
            barycentric_handle_rx,
        }
    }
}

#[async_trait]
impl<T> BarycentricCommunication<T> for BarycentricCommunicator<T>
where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Default + Send + Sync + 'static,
{
    // # Method Description:
    // This method spawns an asynchronous background task responsible for managing the lifecycle 
    // of barycentric agreement rounds. Each spawned task listens for incoming messages or 
    // barycentric reports and processes them according to the Barycentric Agreement protocol. 
    // The method maintains a per-round monitor that tracks message counts, trusted values, 
    // and buddy relationships across participating threads The spawned task ensures
    // rebroadcast of received messages as a barycentric report, collecting Barycentric reports 
    // and useing it to identify “trusted” values, establishing `buddy` processors in the network.
    //
    // # Returns:
    // * `JoinHandle<()>` - A handle to the asynchronous task that continuously listens for 
    //   and processes barycentric communication events in the background.
    fn initialize_barycentric_handle(&mut self) -> JoinHandle<()>{
        println!("initializing barycentric handle...");

        let thread_id = *self.get_id(); 
        let thread_channel = self.get_channels().clone(); 
        let thread_signal_channel = self.get_signal_channels().clone();
        let thread_count = thread_channel.get_channels().len() as u32; 
        let mut receiver = self.take_barycentric_handle_rx(); 

        let faulty_threads = (thread_count - 1) / 3;
        let validity_threshold = thread_count - faulty_threads + 1;
        let agreement_threshold = faulty_threads + 1;

        let mut barycentric_monitor: HashMap<u32, BarycentricRoundMonitor<T>> = HashMap::new();
    
        let handle = tokio::spawn(async move {
            loop  {
                tokio::select! {
                    Some(received_object) = receiver.recv() => {
                        let object: ObjectContent<T>; 
                        if let Ok(message) = Message::read_json(&received_object) {
                            object = ObjectContent::Message(message);
                        } else if let Ok(barycentric_report) = BarycentricReport::read_json(&received_object) {
                            object = ObjectContent::BarycentricReport(barycentric_report);
                        } else {
                            continue
                        }

                        let round_number =  object.get_round_number(); 
                        let protocol_information = object.get_protocol_information().clone();
                        let _ =  barycentric_monitor.entry(round_number).or_insert(BarycentricRoundMonitor::<T>::new(thread_count));

                        let instance = barycentric_monitor.get_mut(&round_number).unwrap(); 
                        let content = &mut instance.content;
                        let state = &mut instance.state;
                        let count = &mut instance.count;

                        match object {
                            ObjectContent::Message(message) => {
                                if !content.messages.contains(&message) {
                                    let id = message.get_id();
                                    content.messages[id as usize] = message; 
                                    count.messages += 1;  
                                    Self::reliable_broadcast_barycentric_report(thread_id, &thread_signal_channel, content, round_number, protocol_information, count).await;
                                }
                                
                                if count.messages >= validity_threshold && state.messages == false {
                                    state.messages = true; 
                                }
                                
                            },
                            ObjectContent::Report(_) => {
                                panic!("Error: received incompatible object type (Report) for barycentric agreement");
                            },
                            ObjectContent::AggregatedReport(_) => {                        
                                panic!("Error: received incompatible object type (AggregatedReport) for barycentric agreement");
                            },
                            ObjectContent::BarycentricReport(barycentric_report) => {     
                                let id = barycentric_report.get_id();

                                content.barycentric_reports[id as usize] = barycentric_report; 
                                // content.barycentric_reports.insert(barycentric_report.get_id() as usize, barycentric_report);
                                count.barycentric_reports += 1;  
                                
                                if state.messages == true && state.trusted == true {
                                    Self::initialize_buddies(thread_id, &mut content.messages, &mut content.buddies, &mut content.barycentric_reports, count);
                                }
                            },
                        }

                        if count.barycentric_reports >= agreement_threshold && state.trusted == false {
                            //confirm approach of using RB barycentric reports to check for a trusted message
                            if Self::initialize_trusted(thread_id, agreement_threshold, count, content).len() > 0 {
                                state.trusted = true;
                            }
                        }

                        if count.buddies >= validity_threshold && state.buddies == false {
                            let protocol_information = String::from("barycentric");
                            let instance_number = 0; 
                            let trusted_messages = Self::initialize_trusted(thread_id, agreement_threshold, count, content).clone();
                            let values = Report::new(ReportType::Witness, protocol_information, thread_id, trusted_messages, None, instance_number, round_number); 
                            thread_channel.send_values(thread_id, values).await;
                            state.buddies = true;
                        } 
                    }
                }
            }
        });
        handle
    } 

    // # Method Description:
    // This asynchronous helper method constructs and reliably broadcasts a `BarycentricReport`
    // to all threads participating in the barycentric agreement round. 
    //
    // # Parameters:
    // * `thread_id` - The ID of the current thread performing the broadcast.
    // * `thread_signal_channel` - The signal channel used to broadcast the barycentric report.
    // * `content` - A mutable reference to the `BarycentricRoundContent` containing the messages
    //   and reports for the current round.
    // * `round_number` - The current barycentric round identifier.
    // * `protocol_information` - A string describing the protocol context ("barycentric").
    // * `count` - A mutable reference to the round counter tracking messages and reports.
    async fn reliable_broadcast_barycentric_report(thread_id: u32, thread_signal_channel: &SignalChannels<T>, content: &mut BarycentricRoundContent<T>, round_number: u32, protocol_information: String, count: &mut BarycentricRoundCount){
        let barycentric_report = Self::create_barycentric_report(thread_id, content, round_number, protocol_information, count); 
        let input = Signal::new(SignalType::Input, ObjectContent::BarycentricReport(barycentric_report.clone()), barycentric_report.get_instance_number(), barycentric_report.get_round_number());
        println!("id: {thread_id}, broadcasting barycentric_report...");
        thread_signal_channel.broadcast_signal(input).await;
    }

    fn get_report_channels(&self) -> &ReportChannels<T> {
        &self.report_channels
    }

    fn take_barycentric_handle_rx(&mut self) -> Receiver<String> {
        self.barycentric_handle_rx.take().unwrap()
    }
}

#[async_trait]
impl<T> ReliableCommunication<T> for BarycentricCommunicator<T> 
where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Default + Send + Sync + 'static,
{
    fn get_signal_channels(&self) -> &SignalChannels<T> {
        &self.signal_channels
    }

    fn take_reliable_handle_rx(&mut self) -> Receiver<String> {
        self.reliable_handle_rx.take().unwrap()
    }

    // # Method Description:
    // This method spawns an asynchronous background task that manages reliable broadcast signals.
    // It listens for incoming signals, updates the state of each instance,
    // broadcasts signals based on protocol thresholds, and delivers messages or reports when conditions are met.
    // # Returns:
    // * A `JoinHandle<()>` representing the spawned async task.
    fn initialize_reliable_handle(&mut self) -> JoinHandle<()>{
        println!("initializing reliable handle...");

        let thread_id = *self.get_id(); 
        let thread_channel = self.get_channels().clone(); 
        let thread_signal_channel = self.get_signal_channels().clone();
        let report_channel = self.get_report_channels().clone(); 
        let thread_count = report_channel.get_handle_channels().len() as u32; 
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
                                    panic!("Error: instance id ({}) already used", instance_id)
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
                                    if signal.get_content().get_protocol_information() == "reliable" {
                                        let channel = ChannelType::MessageChannels(thread_channel.clone());
                                         Self::upon_vote(thread_id, channel, signal).await;
                                    } else {
                                        let channel = ChannelType::ReportChannels(report_channel.clone());
                                        Self::upon_vote(thread_id, channel, signal).await;
                                    }
                                   
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
    async fn upon_input(_thread_id: u32, thread_signal_channel: &SignalChannels<T>, signal: Signal<T>) {
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
    async fn upon_echo(_thread_id: u32, thread_signal_channel: &SignalChannels<T>, signal: Signal<T>) {
        let vote = Signal::new(SignalType::Vote, signal.get_content().clone(), signal.get_instance_number(), signal.get_round_number());
        thread_signal_channel.broadcast_signal(vote).await; 
    }
 

    // # Method Description:
    // As the completion step in the reliable broadcast protocol,
    // handles a `Vote` signal by delivering the final message or barycentric report through the apropriate channel.
    // Panics if the channel or content type does not match expectations.
    //
    // # Parameters:
    // * thread_id - The ID of the current thread processing the signal.
    // * channel - The channel used to deliver the final message (`MessageChannels` or `ReportChannels`).
    // * signal - The received `Vote` signal.
    async fn upon_vote(thread_id: u32, channel: ChannelType<T>, signal: Signal<T>)  {
        let object = signal.get_content().clone(); 

        match channel {
            ChannelType::MessageChannels(thread_channel) => {
                if let ObjectContent::Message(message) = object {
                    thread_channel.send_message(thread_id, message).await;
                }
            },
            ChannelType::ReportChannels(report_channel) => {
                match object {
                    ObjectContent::Message(message) => {
                        report_channel.send_message(thread_id, message).await;     
                    }
                    ObjectContent::Report(_) => {
                        panic!("Error: received incompatible object type (Report) for barycentric agreement");
                    },
                    ObjectContent::AggregatedReport(_) => {
                        panic!("Error: received incompatible object type (AggregatedReport) for barycentric agreement");
                    },
                    ObjectContent::BarycentricReport(barycentric_report) => {
                        report_channel.send_barycentric_report(thread_id, barycentric_report).await;
                    },
                }
            },
        }
    }
}
impl<T> BasicCommunication<T> for BarycentricCommunicator<T> 
where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
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
// This struct represents a report exchanged between threads as part of the barycentric agreement 
// protocol. Each report contains the current messages collected by a thread for a specific 
// consensus instance and round.
//
// # Fields:
// * protocol_information - A string identifying the protocol context (e.g., "barycentric").
// * id - The ID of the thread that created the report.
// * messages - A vector of `Message`s collected by this thread for the current round.
// * instance_number - The consensus instance number associated with this report.
// * round_number - The round number of the protocol in which this report was generated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct BarycentricReport<T>{
    protocol_information: String, 
    id: u32, 
    messages: Vec<Message<T>>, 
    instance_number: u32,
    round_number: u32
}

impl<T> BarycentricReport<T> 
where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    pub fn get_protocol_information(&self) -> &String {
        &self.protocol_information
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_messages(&self) -> &Vec<Message<T>> {
        &self.messages
    }

    pub fn get_instance_number(&self) -> u32 {
        self.instance_number
    }

    pub fn get_round_number(&self) -> u32 {
        self.round_number
    }

    pub fn new(protocol_information: String, id: u32, messages: Vec<Message<T>>, instance_number: u32, round_number: u32) -> Self {
        Self {
            protocol_information,
            id, 
            messages,
            instance_number,
            round_number
        }
    }
}

impl<T> JsonConversion<BarycentricReport<T>> for BarycentricReport<T> where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{}

// # Struct Description:
// This struct monitors the state of a single round in the barycentric agreement protocol,
// aggregating the collected content, current state flags, and count of received messages and reports.
//
// # Fields:
// * content - A `BarycentricRoundContent` instance containing messages, reports, and buddy flags for this round.
// * state - A `BarycentricRoundState` instance tracking which thresholds (messages, trusted, buddies) have been reached.
// * count - A `BarycentricRoundCount` instance keeping numerical counts of messages, reports, and buddies.
pub struct BarycentricRoundMonitor<T> 
where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Default + Send + Sync + 'static,
{
    pub content: BarycentricRoundContent<T>,
    pub state: BarycentricRoundState,
    pub count: BarycentricRoundCount, 
}

impl<T> BarycentricRoundMonitor<T> 
where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Default + Send + Sync + 'static,
{
    pub fn new(thread_count: u32) -> Self {
        let content = BarycentricRoundContent::new(thread_count);
        let state = BarycentricRoundState::new();
        let count = BarycentricRoundCount::new();
        Self {
            content,
            state,
            count
        }
    }
}

// # Struct Description:
// This struct tracks the boolean state of a barycentric round to indicate if key thresholds have been met.
//
// # Fields:
// * messages - `true` if the message collection threshold has been reached.
// * trusted - `true` if the trusted message threshold has been reached.
// * buddies - `true` if the buddy agreement threshold has been reached.
pub struct BarycentricRoundState {
    pub messages: bool,
    pub trusted: bool,
    pub buddies: bool,
}

impl BarycentricRoundState {
    pub fn new() -> Self {
        let messages = false;
        let trusted = false;  
        let buddies = false; 
        Self {
            messages,
            trusted,
            buddies
        }
    }
}

// # Struct Description:
// This struct holds the content of a single barycentric round, including all messages, collected reports, and buddy flags.
//
// # Fields:
// * messages - A vector of `Message`s collected in this round.
// * barycentric_reports - A vector of `BarycentricReport`s received in this round.
// * buddies - A vector of booleans representing whether each peer is considered a buddy for this round.
pub struct BarycentricRoundContent<T> 
where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
{
    pub messages: Vec<Message<T>>,
    pub barycentric_reports: Vec<BarycentricReport<T>>,
    pub buddies: Vec<bool>
}

impl<T> BarycentricRoundContent<T> 
where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Default + Send + Sync + 'static,
{
    pub fn new(thread_count: u32) -> Self {
        let initial_message = Message::new("".to_string(), 0, T::default(), None, None, 0); 
        let initial_report = BarycentricReport::new("".to_string(), 0, vec![initial_message.clone()], 0, 0);
        let messages = vec![initial_message; thread_count as usize];
        let barycentric_reports = vec![initial_report; thread_count as usize];
        let buddies = vec![false; thread_count as usize];

        Self {
            messages,
            barycentric_reports,
            buddies
        }
    }
}

// # Struct Description:
// This struct keeps numerical counts for tracking progress in a barycentric round.
//
// # Fields:
// * messages - The number of messages received in this round.
// * barycentric_reports - The number of barycentric reports received in this round.
// * buddies - The number of confirmed buddies in this round.
pub struct BarycentricRoundCount {
    pub messages: u32,
    pub barycentric_reports: u32, 
    pub buddies: u32
}

impl BarycentricRoundCount {
    pub fn new() -> Self {
        let messages = 0; 
        let barycentric_reports = 0; 
        let buddies = 0; 
        Self {
            messages,
            barycentric_reports,
            buddies
        }
    }
}

