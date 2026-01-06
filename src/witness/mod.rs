use core::panic;
use std::{vec, fmt::Debug, hash::Hash, collections::{HashMap, HashSet}, marker::PhantomData};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use tokio::{task::JoinHandle, sync::mpsc::{self, Receiver, Sender}};
use async_trait::async_trait; 

use crate::{barycentric_agreement::BarycentricReport,  basic::{BasicCommunication, BasicQueues, Message, MessageChannels, RecvObject}}; 
use crate::reliable::{ReliableCommunication, Signal, SignalType, ChannelType, ObjectContent, SignalChannels, ReliableInstanceMonitor}; 
use crate::aggregated_witness::{AggregatedReport};
use crate::json::{JsonConversion};

// # Trait Description:
// This trait defines the behavior for threads participating in a witness-based reliable broadcast protocol.
// It extends `ReliableCommunication` by providing methods to handle witness report creation, broadcasting,
// collection, and task management for asynchronous witness communication. 
#[async_trait]
pub trait WitnessCommunication<T>: ReliableCommunication<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
{
    // # Function Description:
    // This function iterates through all reports in the current witness round and converts eligible reports
    // into witnesses based on the contained messages matching the expected values.
    // # Parameters:
    // * thread_id - The ID of the current thread processing the reports.
    // * count - A mutable reference to the `WitnessRoundCount` tracking the number of witnesses.
    // * content - A mutable reference to the `WitnessRoundContent` containing reports and witnesses.
    fn update_witnesses(thread_id: u32, count: &mut WitnessRoundCount, content: &mut WitnessRoundContent<T>) {
        for report in &mut content.reports {
            if report.get_report_type() == &ReportType::Report {
                Self::initialize_witnesses(thread_id, report, &mut content.witnesses, count, content.values.clone());
            }
        }
    }

    // # Function Description:
    // This function checks if a report’s messages are a subset of expected values and, if so, converts it
    // into a witness, adding it to the list of witnesses and updating the count.
    // # Parameters:
    // * thread_id - The ID of the current thread processing the report.
    // * report - A mutable reference to the `Report` to potentially convert into a witness.
    // * witnesses - A mutable vector of `Report`s representing collected witnesses.
    // * count - A mutable reference to the `WitnessRoundCount` to update witness count.
    // * values - A vector of `Message`s representing expected values for this round.
    fn initialize_witnesses(thread_id: u32, report: &mut Report<T>, witnesses: &mut Vec<Report<T>>, count: &mut WitnessRoundCount, values: Vec<Message<T>>) {
        let values_set: HashSet<Message<T>> = values.into_iter().collect();
        let report_set: HashSet<Message<T>> = report.get_messages().clone().into_iter().collect();

        if report_set.is_subset(&values_set) {
            report.report_type = ReportType::Witness;
            witnesses.push(report.clone());
            println!("id: {thread_id}: converted report by id: {} to a witness", report.get_id());
            count.witnesses += 1; 
        }       
    }

    // # Method Description:
    // This method initiates a witness broadcast by wrapping a message into a signal and sending it to all participants.
    // # Parameters:
    // * message - The content to broadcast as a String.
    // * round_number - The round number associated with this witness broadcast.
    // # Returns:
    // * A future that broadcasts the signal to all signal receivers.
    fn witness_broadcast(&mut self, message: T, round_number: u32) -> impl Future<Output = ()> {
        let protocol_information = String::from("witness");
        let instance_number = 0; 
        let sent_message = Message::new(protocol_information, *self.get_id(), message, None, Some(instance_number), round_number);
        let input = Signal::new(SignalType::Input, ObjectContent::Message(sent_message), instance_number, round_number);
        self.get_signal_channels().broadcast_signal(input)
    }

    // # Method Description:
    // This method collects all witness reports for the given round by retrieving a collection from the local queue.
    // # Parameters:
    // * round_number - The round number to collect witness reports.
    // # Returns:
    // * A vector of `Message`s contained in the collected witness report.
    async fn witness_collect(&mut self, round_number: u32) -> Vec<Message<T>>{
        let protocol_information = String::from("witness");
        let thread_id = self.get_id().clone();

        match self.get_queues().basic_recv(Some(thread_id), protocol_information, Some(0), round_number).await {
            RecvObject::Message(_) => {panic!("Error: retreived Message instead of Vec<Message>")},
            RecvObject::Collection(report) => {
                println!("witness collected: {:?}", &report.get_messages());    
                let collection = report.get_messages().clone();
                return collection;
            },
        }
    }

    // # Method Description:
    // This method terminates the asynchronous task responsible for handling witness messages.
    // # Parameters:
    // * witness_handle - The `JoinHandle<()>` representing the spawned witness task to terminate.
    fn terminate_witness_handle(&self, witness_handle: JoinHandle<()>) {
        println!("id: {}, terminating witness_handle...", self.get_id());
        witness_handle.abort();
    }

    async fn reliable_broadcast_report(thread_id: u32, thread_signal_channel: &SignalChannels<T>, content: &mut WitnessRoundContent<T>, dimension: Option<u32>, round_number: u32, protocol_information: String); 
    fn initialize_witness_handle(&mut self) -> JoinHandle<()>; 
    fn take_witness_handle_rx(&mut self) -> Receiver<String>;
    fn get_report_channels(&self) -> &ReportChannels<T>;

}

// # Struct Description:
// This struct manages a collection of `WitnessCommunicator` instances, each representing a thread
// participating in witness-based reliable broadcast. It handles initialization of communication
// channels for both reliable and witness-specific messaging and provides access to individual communicators.
// # Fields:
// * witness_communicators - A vector containing all `WitnessCommunicator` instances managed by this hub.
pub struct WitnessHub<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    witness_communicators: Vec<WitnessCommunicator<T>>
}
 
impl<T> WitnessHub<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    pub fn new(transmitters: Vec<Sender<String>>, mut receivers: Vec<Receiver<String>>, thread_count: u32) -> Self {  
        let mut witness_communicators = vec![];
        let mut reliable_handle_transmitters = vec![];
        let mut reliable_handle_receivers = vec![];

        let mut witness_handle_transmitters = vec![];
        let mut witness_handle_receivers = vec![];

        for _ in 0..(thread_count) {
            let (reliable_handle_tx, reliable_handle_rx) = mpsc::channel(256); 
            let (witness_handle_tx, witness_handle_rx) = mpsc::channel(256); 

            reliable_handle_transmitters.push(reliable_handle_tx);
            reliable_handle_receivers.push(reliable_handle_rx);

            witness_handle_transmitters.push(witness_handle_tx);
            witness_handle_receivers.push(witness_handle_rx);
        }
        
        for i in 0..(thread_count) {
            let reliable_handle_rx = reliable_handle_receivers.remove(0);
            let witness_handle_rx = witness_handle_receivers.remove(0);
            let rx: Receiver<String> = receivers.remove(0);
            witness_communicators.push(WitnessCommunicator::new(transmitters.clone(), rx, 
                thread_count, i as u32, reliable_handle_transmitters.clone(), reliable_handle_rx, witness_handle_transmitters.clone(), witness_handle_rx));
        }
        
        Self {
            witness_communicators
        }
    }
 
    pub fn create_witness_communicator(&mut self) -> WitnessCommunicator<T>{
        self.witness_communicators.remove(0)
    }
 }

// # Struct Description:
// This struct represents a single thread in a witness-based reliable broadcast system. 
// It extends basic communication with both reliable and witness-specific protocols, maintaining
// dedicated channels for standard messages, protocol-level signals, and report aggregation.
//
// # Fields:
// * id - The unique identifier for this thread.
// * basic_channels - Handles standard inter-thread messaging.
// * signal_channels - Handles protocol-specific signal broadcasting (e.g., Input, Echo, Vote).
// * report_channels - Handles communication of report objects for witness verification.
// * queues - Stores incoming messages for this thread.
// * reliable_handle_rx - A receiver for incoming reliable broadcast signals.
// * witness_handle_rx - A receiver for incoming witness broadcast signals.
pub struct WitnessCommunicator<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    id: u32, 
    basic_channels: MessageChannels<T>, 
    signal_channels: SignalChannels<T>, 
    report_channels: ReportChannels<T>,
    queues: BasicQueues<T>,
    reliable_handle_rx: Option<Receiver<String>>, 
    witness_handle_rx: Option<Receiver<String>>, 
}

impl<T> WitnessCommunicator<T> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    fn new(transmitters: Vec<Sender<String>>, receiver: Receiver<String>, 
            thread_count: u32, id: u32, reliable_handle_transmitters: Vec<Sender<String>>, reliable_handle_rx: Receiver<String>, witness_handle_transmitters: Vec<Sender<String>>, witness_handle_rx: Receiver<String>) -> Self {
        let basic_channels = MessageChannels::new(transmitters.clone());
        let signal_channels = SignalChannels::new(reliable_handle_transmitters.clone());
        let report_channels = ReportChannels::new(witness_handle_transmitters.clone());
        let queues = BasicQueues::new(receiver, thread_count);
        let reliable_handle_rx = Some(reliable_handle_rx);
        let witness_handle_rx = Some(witness_handle_rx);

        Self {
            id, 
            basic_channels,
            signal_channels,
            report_channels,
            queues,
            reliable_handle_rx,
            witness_handle_rx,
        }
    }
}

#[async_trait]
impl<T> WitnessCommunication<T> for WitnessCommunicator<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
{
    // # Method Description:
    // This method spawns an asynchronous background task that listens for incoming witness messages and reports.
    // It tracks the state of each round, updates collected values and reports, converts eligible reports to witnesses,
    // and triggers reliable broadcasts or sends values to the message channel when thresholds are met.
    //
    // # Returns:
    // * A `JoinHandle<()>` representing the spawned asynchronous task.
    fn initialize_witness_handle(&mut self) -> JoinHandle<()>{
        println!("initializing witness handle...");

        let thread_id = *self.get_id(); 
        let thread_channel = self.get_channels().clone(); 
        let thread_signal_channel = self.get_signal_channels().clone();
        let thread_count = thread_channel.get_channels().len() as u32; 
        let mut receiver = self.take_witness_handle_rx(); 
        let faulty_threads = (thread_count - 1) / 3;
        let validity_threshold = thread_count - faulty_threads + 1;
        let mut witness_monitor: HashMap<u32, WitnessRoundMonitor<T>> = HashMap::new();
    
        let handle = tokio::spawn(async move {
            loop  {
                tokio::select! {
                    Some(received_object) = receiver.recv() => {
                        let object: ObjectContent<T>; 
                        if let Ok(message) = Message::read_json(&received_object) {
                            object = ObjectContent::Message(message);
                        } else if let Ok(report) = Report::read_json(&received_object) {
                            object = ObjectContent::Report(report);
                        } else {
                            continue
                        }

                        let round_number =  object.get_round_number(); 
                        let protocol_information = object.get_protocol_information().clone();
                        let _ =  witness_monitor.entry(round_number).or_insert(WitnessRoundMonitor::new());

                        let instance = witness_monitor.get_mut(&round_number).unwrap(); 
                        let content = &mut instance.content;
                        let state = &mut instance.state;
                        let count = &mut instance.count;

                        match object {
                            ObjectContent::Message(message) => {
                                if !content.values.contains(&message) {
                                    content.values.push(message);
                                    count.values += 1;  
                                    if count.values > validity_threshold {
                                        Self::update_witnesses(thread_id, count, content);
                                    }
                                }
                            },
                            ObjectContent::Report(report) => {
                                if !content.reports.contains(&report) {
                                    content.reports.push(report);
                                    count.reports += 1;  
                                    let report = content.reports.get_mut((count.reports - 1) as usize).unwrap(); 
                                    Self::initialize_witnesses(thread_id, report, &mut content.witnesses, count, content.values.clone()); 
                                }
                            },
                            ObjectContent::AggregatedReport(_) => {                        
                                panic!("Error: received incompatible object type (AggregatedReport) for witness broadcast");
                            },
                            ObjectContent::BarycentricReport(_) => {                        
                                panic!("Error: received incompatible object type (BarycentricReport) for witness broadcast");
                            },
                        }

                        if count.values >= validity_threshold && state.report == false {
                            Self::reliable_broadcast_report(thread_id, &thread_signal_channel, content, None, round_number, protocol_information).await;
                            state.report = true; 
                        }

                        if count.witnesses >= validity_threshold && state.witnesses == false {
                            let protocol_information = String::from("witness");
                            let instance_number = 0; 
                            let values = Report::new(ReportType::Witness, protocol_information, thread_id, content.values.clone(), None, instance_number, round_number); 
                            thread_channel.send_values(thread_id, values).await;
                            state.witnesses = true; 
                        }
                    }
                }
            }
        });
        handle
    } 

    // # Method Description:
    // This asynchronous method broadcasts a `Report` containing collected values for a witness round
    // to all participants via signal channels. It wraps the report in an `Input` signal to initiate
    // the protocol for reliable broadcast of report data.
    //
    // # Parameters:
    // * thread_id - The ID of the current thread broadcasting the report.
    // * thread_signal_channel - The `SignalChannels` instance used to send the report to all participants.
    // * content - The `WitnessRoundContent` containing collected messages for the report.
    // * round_number - The current round number for the witness collection.
    // * protocol_information - A string representing the protocol type.
    //
    // # Returns:
    // * A future that broadcasts the report to all signal receivers.`
    async fn reliable_broadcast_report(thread_id: u32, thread_signal_channel: &SignalChannels<T>, content: &mut WitnessRoundContent<T>, _dimension: Option<u32>, round_number: u32, protocol_information: String){
        let protocol_information = protocol_information;
        let instance_number = 0; 
        let report = Report::new(ReportType::Report, protocol_information, thread_id, content.values.clone(), None, instance_number, round_number); 
        let input = Signal::new(SignalType::Input, ObjectContent::Report(report.clone()), report.get_instance_number(), report.get_round_number());
        println!("id: {thread_id}, broadcasting report...");
        thread_signal_channel.broadcast_signal(input).await;
    }

    fn get_report_channels(&self) -> &ReportChannels<T> {
        &self.report_channels
    }

    fn take_witness_handle_rx(&mut self) -> Receiver<String> {
        self.witness_handle_rx.take().unwrap()
    }
}

#[async_trait]
impl<T> ReliableCommunication<T> for WitnessCommunicator<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
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
    // handles a `Vote` signal by delivering the final message or report through the apropriate channel.
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
                    ObjectContent::Report(report) => {
                        report_channel.send_report(thread_id, report).await;
                    },
                    ObjectContent::AggregatedReport(_) => {
                        panic!("Error: received incompatible object type (AggregatedReport) for witness broadcast");
                    },
                    ObjectContent::BarycentricReport(_) => {
                        panic!("Error: received incompatible object type (BarycentricReport) for witness broadcast");
                    }
                }
            },
        }
    }
}

impl<T> MessageChannels<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    // # Method Description:
    // This method sends a `Report` (a collection of messages or values) to a specific thread
    // through its corresponding message channel.
    //
    // # Parameters:
    // * id - The ID of the target thread to receive the report.
    // * values - The `Report` instance containing messages or values to be sent.
    //
    // # Returns:
    // * A future that completes once the report is sent.
    pub(crate) fn send_values(&self, id: u32, values: Report<T>) -> impl Future<Output = ()>{
        async move {
            match self.get_channels().get(id as usize) {
                Some(channel) => {
                    println!("id: {id}, delivering values...");
                    let _ = channel.send(values.write_json()).await;
                },
                None => panic!("Error: received incompatible object type (aggregated_report) for witness broadcast"),
            }
        }
    }
}

impl<T> BasicCommunication<T> for WitnessCommunicator<T>
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
// This struct manages a set of message channels specifically for sending reports
// and aggregated reports between threads in a witness-based reliable communication protocol.
//
// # Fields:
// * witness_handle_transmitters - A vector of `Sender<String>` channels used to send serialized reports to target threads.
#[derive(Clone)]
pub struct ReportChannels<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash, 
{
   witness_handle_transmitters: Vec<Sender<String>>,
    _marker: PhantomData<T>,
}

impl<T> ReportChannels<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    // # Method Description:
    // This method sends a single `Message` to a specific thread via its corresponding witness channel.
    //
    // # Parameters:
    // * id - The ID of the target thread.
    // * message - The `Message` instance to send.
    //
    // # Returns:
    // * A future that completes once the message is sent.
    pub(crate) fn send_message(&self, id: u32, message: Message<T>) -> impl Future<Output = ()>{
        async move {
            match self.get_handle_channels().get(id as usize) {
                Some(channel) => {
                    let _ = channel.send(message.write_json()).await;
                },
                None => panic!("Error: failed to find channel"),
            }
        }
    }

    // # Method Description:
    // This method sends a `Report` of type `Report` to a specific thread. Panics if the report type is `Witness`.
    //
    // # Parameters:
    // * id - The ID of the target thread.
    // * report - The `Report` instance to send.
    //
    // # Returns:
    // * A future that completes once the report is sent.
    pub(crate) fn send_report(&self, id: u32, report: Report<T>) -> impl Future<Output = ()>{
        async move {
            match self.get_handle_channels().get(id as usize) {
                Some(channel) => {
                    match &report.get_report_type() {
                        ReportType::Report => {
                            let _ = channel.send(report.write_json()).await;
                        },
                        ReportType::Witness => {
                            panic!("Error: received incompatible object type (witness) for reliable delivery");
                        },
                    }
                },
                None => panic!("Error: failed to find channel"),
            }
        }
    }

    // # Method Description:
    // This method sends an `AggregatedReport` of type `Report` to a specific thread. Panics if the report type is `Witness`.
    //
    // # Parameters:
    // * id - The ID of the target thread.
    // * aggregated_report - The `AggregatedReport` instance to send.
    //
    // # Returns:
    // * A future that completes once the aggregated report is sent.
    pub(crate) fn send_aggregated_report(&self, id: u32, aggregated_report: AggregatedReport<T>) -> impl Future<Output = ()>{
        async move {
            match self.get_handle_channels().get(id as usize) {
                Some(channel) => {
                    match &aggregated_report.get_report_type() {
                        ReportType::Report => {
                            let _ = channel.send(aggregated_report.write_json()).await;
                        },
                        ReportType::Witness => {
                            panic!("Error: received incompatible object type (witness) for reliable delivery");
                        },
                    }
                },
                None => panic!("Error: failed to find channel"),
            }
        }
    }

    pub(crate) fn send_barycentric_report(&self, id: u32, barycentric_report: BarycentricReport<T>) -> impl Future<Output = ()>{
        async move {
            match self.get_handle_channels().get(id as usize) {
                Some(channel) => {
                    let _ = channel.send(barycentric_report.write_json()).await;
                },
                None => panic!("Error: failed to find channel"),
            }
        }
    }

    pub fn get_handle_channels(&self) -> &Vec<Sender<String>> {
       &self.witness_handle_transmitters
    }

    pub fn new(witness_handle_transmitters: Vec<Sender<String>>) -> Self {
       Self {
           witness_handle_transmitters,
           _marker: PhantomData,
       }
    }
}

// # Enum Description:
// This enum represents the type of report being transmitted between threads in 
// a witness-based reliable communication protocol.
//
// # Variants:
// * Report - A standard report containing values collected from threads, 
// used for reliable broadcast or consensus.
// * Witness - A report that has been validated and converted into a witness, 
// indicating that the contained values have been verified by a thread.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ReportType{
    Report,
    Witness,
}

// # Struct Description:
// This struct represents a report exchanged between threads as part of the witness-based reliable communication protocol.
// Reports can be standard reports containing collected messages or validated witnesses.
//
// # Fields:
// * report_type - The type of the report, either `Report` or `Witness`.
// * protocol_information - A string identifying the protocol or message type.
// * id - The ID of the thread that created the report.
// * messages - A vector of `Message`s contained in this report.
// * instance_number - The consensus instance associated with this report.
// * round_number - The round number of the protocol in which this report was created.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Report<T>
{
    pub report_type: ReportType,
    protocol_information: String, 
    id: u32, 
    messages: Vec<Message<T>>, 
    dimension: Option<u32>,
    instance_number: u32,
    round_number: u32
}

impl<T> Report<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    pub fn get_report_type(&self) -> &ReportType {
        &self.report_type
    }

    pub fn get_protocol_information(&self) -> &String {
        &self.protocol_information
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_messages(&self) -> &Vec<Message<T>> {
        &self.messages
    }

    pub fn get_dimension(&self) -> Option<u32> {
        self.dimension
    }

    pub fn get_instance_number(&self) -> u32 {
        self.instance_number
    }

    pub fn get_round_number(&self) -> u32 {
        self.round_number
    }

    pub fn new(report_type: ReportType, protocol_information: String, id: u32, messages: Vec<Message<T>>, dimension: Option<u32>,instance_number: u32, round_number: u32) -> Self {
        Self {
            report_type,
            protocol_information,
            id, 
            messages,
            dimension, 
            instance_number,
            round_number
        }
    }
}

impl<T> JsonConversion<Report<T>> for Report<T> 
where
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{}

// # Struct Description:
// This struct monitors the progress of a single witness round, tracking its content, state, and counts.
//
// # Fields:
// * content - The messages, reports, and aggregated reports/witnesses collected in this round.
// * state - The current state flags indicating which milestones have been reached.
// * count - Counters for how many messages, reports, and witnesses have been observed.
pub struct WitnessRoundMonitor<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    pub content: WitnessRoundContent<T>,
    pub state: WitnessRoundState,
    pub count: WitnessRoundCount, 
}

impl<T> WitnessRoundMonitor<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    pub fn new() -> Self {
        let content = WitnessRoundContent::new();
        let state = WitnessRoundState::new();
        let count = WitnessRoundCount::new();
        Self {
            content,
            state,
            count
        }
    }
}
// # Struct Description:
// This struct represents the completion state of a witness round.
//
// # Fields:
// * report - Indicates whether a report has been reliably broadcasted.
// * witnesses - Indicates whether witnesses have been collected.
// * aggregated_witnesses - Indicates whether aggregated witnesses have been collected.
pub struct WitnessRoundState {
    pub report: bool,
    pub witnesses: bool,
    pub aggregated_witnesses: bool
}

impl WitnessRoundState {
    pub fn new() -> Self {
        let report = false; 
        let witnesses = false;
        let aggregated_witnesses = false;
        Self {
            report,
            witnesses,
            aggregated_witnesses
        }
    }
}

// # Struct Description:
// This struct holds all collected data during a witness round.
//
// # Fields:
// * values - Messages collected in the current round.
// * reports - Reports received from threads.
// * witnesses - Reports validated as witnesses.
// * aggregated_reports - Aggregated reports collected in the round.
// * aggregated_witnesses - Aggregated witness reports collected in the round.
pub struct WitnessRoundContent<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    pub values: Vec<Message<T>>,
    pub reports: Vec<Report<T>>,
    pub witnesses: Vec<Report<T>>,
    pub barycentric_values: Vec<Message<Vec<T>>>,
    pub barycentric_reports: Vec<Report<Vec<T>>>,
    pub barycentric_witnesses: Vec<Report<Vec<T>>>,
    pub aggregated_reports: Vec<AggregatedReport<T>>,
    pub aggregated_witnesses: Vec<AggregatedReport<T>>,
    pub dimension: Option<u32>, 
    pub instance_number: u32, 
}

impl<T> WitnessRoundContent<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    pub fn new() -> Self {
        let values = vec![];
        let reports = vec![];
        let witnesses = vec![];
        let barycentric_values = vec![];
        let barycentric_reports = vec![];
        let barycentric_witnesses = vec![];
        let aggregated_reports = vec![];
        let aggregated_witnesses = vec![];
        let dimension = None;
        let instance_number = 0; 

        Self {
            values,
            reports,
            witnesses,
            barycentric_values,
            barycentric_reports,
            barycentric_witnesses,
            aggregated_reports,
            aggregated_witnesses,
            dimension,
            instance_number
        }
    }
}

// # Struct Description:
// This struct counts the number of elements observed during a witness round.
//
// # Fields:
// * values - Count of messages collected.
// * reports - Count of reports received.
// * witnesses - Count of validated witness reports.
// * aggregated_reports - Count of aggregated reports received.
// * aggregated_witnesses - Count of aggregated witnesses collected.
pub struct WitnessRoundCount {
    pub values: u32,
    pub reports: u32,
    pub witnesses: u32,
    pub aggregated_reports: u32,
    pub aggregated_witnesses: u32
}

impl WitnessRoundCount {
    pub fn new() -> Self {
        let values = 0; 
        let reports = 0; 
        let witnesses = 0; 
        let aggregated_reports = 0; 
        let aggregated_witnesses = 0; 
        Self {
            values,
            reports,
            witnesses,
            aggregated_reports,
            aggregated_witnesses
        }
    }
}

impl<T> JsonConversion<Vec<Message<T>>> for Vec<Message<T>> 
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{}
