use core::panic;
use std::{vec, fmt::Debug, hash::Hash, collections::{HashMap, HashSet}};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use tokio::{task::JoinHandle, sync::mpsc::{self, Receiver, Sender}};
use async_trait::async_trait; 

use crate::{basic::{BasicCommunication, BasicQueues, Message, MessageChannels, RecvObject}}; 
use crate::reliable::{ReliableCommunication, Signal, SignalType, ChannelType, ObjectContent, SignalChannels, ReliableInstanceMonitor}; 
use crate::witness::{WitnessCommunication, WitnessRoundMonitor, WitnessRoundCount, WitnessRoundContent, Report, ReportType, ReportChannels}; 
use crate::json::{JsonConversion};

// # Struct Description:
// The struct initializes per-thread communication channels and coordinates 
// the creation of communicators that handle both raw and aggregated messages
// to enable multi-level witness and reliable broadcast communication across 
// asynchronous threads.
//
// # Fields:
// * aggregated_witness_communicators - A vector of `AggregatedWitnessCommunicator` 
//   instances, each assigned to a specific thread for handling message exchange.
//
pub struct AggregatedWitnessHub<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    aggregated_witness_communicators: Vec<AggregatedWitnessCommunicator<T>>
}
 
impl<T> AggregatedWitnessHub<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{
    pub fn new(transmitters: Vec<Sender<String>>, mut receivers: Vec<Receiver<String>>, thread_count: u32) -> Self {  
        let mut aggregated_witness_communicators = vec![];
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
            aggregated_witness_communicators.push(AggregatedWitnessCommunicator::new(transmitters.clone(), rx, 
                thread_count, i as u32, reliable_handle_transmitters.clone(), reliable_handle_rx, witness_handle_transmitters.clone(), witness_handle_rx));
        }
        
        Self {
            aggregated_witness_communicators
        }
    }
 
    pub fn create_aggregated_witness_communicator(&mut self) -> AggregatedWitnessCommunicator<T>{
        self.aggregated_witness_communicators.remove(0)
    }
 }

// # Struct Description:
// This struct manages all communication primitives required 
// by a single thread to participate in aggregated witness broadcast protocols. 
// It encapsulates multiple types of channels (basic, signal, and report) along with 
// message queues to handle both direct and aggregated message delivery.
//
// # Fields:
// * id - Unique identifier for the communicator’s owning thread.
// * basic_channels - `MessageChannels` used for standard message broadcasting.
// * signal_channels - `SignalChannels` for handling reliable-broadcast signaling 
//   across threads.
// * report_channels - `ReportChannels` for exchanging witness and aggregated 
//   witness reports.
// * queues - `BasicQueues` instance managing per-thread message queues.
// * reliable_handle_rx - A receiver for handling incoming 
//   reliable broadcast messages.
// * witness_handle_rx - A receiver for handling incoming 
//   witness report messages.
pub struct AggregatedWitnessCommunicator<T>
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

impl<T> AggregatedWitnessCommunicator<T> 
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

// # Trait Description: 
// This trait defines the core operations required for executing the aggregated witness protocol
// across distributed threads. This trait abstracts the functionality of broadcasting,
// collecting, updating, and rebroadcasting witness information in multi-round protocols.
#[async_trait]
pub trait AggregatedWitnessCommunication<T>: WitnessCommunication<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
{
    // # Function Description: 
    // This function broadcasts a message in the "aggregated witness" protocol by wrapping it in a `Message`
    // and embedding it into an input `Signal`. The signal is then reliably broadcast to all peers
    // through the communicator’s `SignalChannels`.
    //
    // # Parameters:
    // * message - The raw string payload to broadcast.
    // * round_number - The round of the protocol this broadcast belongs to.
    //
    // # Returns:
    // * A future that completes once the broadcast has been enqueued.
    fn aggregated_witness_broadcast(&mut self, message: T, round_number: u32) -> impl Future<Output = ()> {
        let protocol_information = String::from("aggregated witness");
        let instance_number = 0; 
        let sent_message = Message::new(protocol_information, *self.get_id(), message, None, Some(instance_number), round_number);
        let input = Signal::new(SignalType::Input, ObjectContent::Message(sent_message), instance_number, round_number);
        self.get_signal_channels().broadcast_signal(input)
    }

    // #Function Description: 
    // This function collects the messages delivered for the specified round of the aggregated witness communication protocol 
    // from the communicator’s `BasicQueues`.
    // Ensures the object retrieved is a collection (`Vec<Message>`) and returns it; otherwise, panic. 
    //
    // # Parameters:
    // * round_number - The round of the protocol this collection belongs to.
    //
    // # Returns:
    // * A `Vec<Message>` containing the collected witness messages.
    async fn aggregated_witness_collect(&mut self, round_number: u32) -> Vec<Message<T>>{
        let protocol_information = String::from("aggregated witness");
        let thread_id = self.get_id().clone();

        match self.get_queues().basic_recv(Some(thread_id), protocol_information, Some(0), round_number).await {
            RecvObject::Message(_) => {panic!("Error: retreived Message instead of Vec<Message>")},
            RecvObject::Collection(report) => {
                println!("aggregated witness collected: {:?}", &report.get_messages());    
                let collection = report.get_messages().clone();
                return collection;
            },
        }
    }

    // # Function Description:
    // This function iterates over all aggregated reports in the given round content and attempts to 
    // upgrade them into aggregated witnesses if their component reports are present in 
    // the witness set.
    //
    // # Parameters:
    // * thread_id - The ID of the calling thread.
    // * count - Mutable reference to the round’s count tracker (`WitnessRoundCount`).
    // * content - Mutable reference to the round’s content tracker (`WitnessRoundContent`).
    fn update_aggregated_witnesses(thread_id: u32, count: &mut WitnessRoundCount, content: &mut WitnessRoundContent<T>) {
        for aggregated_report in &mut content.aggregated_reports {
            if aggregated_report.get_report_type() == &ReportType::Report {
                Self::initialize_aggregated_witnesses(thread_id, aggregated_report, &mut content.aggregated_witnesses, count, content.witnesses.clone());
            }
        }
    }

    // # Function Description: 
    // This function checks whether the reports within a given aggregated report form a subset of the 
    // known witnesses. If so, converts the aggregated report’s type into `Witness`, 
    // adds it to the aggregated witnesses list, and updates the count.
    //
    // # Parameters:
    // * thread_id - The ID of the calling thread.
    // * aggregated_report - The mutable aggregated report candidate.
    // * aggregated_witnesses - The collection of aggregated witnesses to update.
    // * count - Mutable reference to the round’s count tracker.
    // * witnesses - The set of known witness reports for comparison.
    fn initialize_aggregated_witnesses(thread_id: u32, aggregated_report: &mut AggregatedReport<T>, aggregated_witnesses: &mut Vec<AggregatedReport<T>>,count: &mut WitnessRoundCount, witnesses: Vec<Report<T>>) {
        let witnesses_set: HashSet<Report<T>> = witnesses.into_iter().collect();
        let aggregated_report_set: HashSet<Report<T>> = aggregated_report.get_reports().clone().into_iter().collect();

        if aggregated_report_set.is_subset(&witnesses_set) {
            aggregated_report.report_type = ReportType::Witness;
            aggregated_witnesses.push(aggregated_report.clone());

            println!("id: {thread_id}: converted aggregated report by id: {} to an aggregated witness", aggregated_report.get_id());
            count.aggregated_witnesses += 1; 
        }       
    }

    // # Function Description
    // This function constructs a new aggregated report from the thread’s collected witnesses 
    // and reliably broadcasts it using the provided `SignalChannels`.
    //
    // # Parameters:
    // * thread_id - The ID of the calling thread.
    // * thread_signal_channel - Reference to the thread’s signal channels for broadcasting.
    // * content - Mutable reference to the round’s content.
    // * round_number - The round of the protocol this broadcast belongs to.
    //
    // # Returns:
    // * A future that completes once the broadcast has been enqueued.
    async fn reliable_broadcast_aggregated_report(thread_id: u32, thread_signal_channel: &SignalChannels<T>, content: &mut WitnessRoundContent<T>, round_number: u32){
        let protocol_information = String::from("aggregated witness");
        let instance_number = 0; 
        let aggregated_report = AggregatedReport::new(ReportType::Report, protocol_information, thread_id, content.witnesses.clone(), instance_number, round_number); 
        let input = Signal::new(SignalType::Input, ObjectContent::AggregatedReport(aggregated_report.clone()), aggregated_report.get_instance_number(), aggregated_report.get_round_number());
        println!("id: {thread_id}, broadcasting aggregated report...");
        thread_signal_channel.broadcast_signal(input).await;
    }
}

impl<T> AggregatedWitnessCommunication<T> for AggregatedWitnessCommunicator<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
{}

#[async_trait]
impl<T> WitnessCommunication<T> for AggregatedWitnessCommunicator<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash + Send + Sync + 'static,
{

    // # Method Description: 
    // This method spawns an asynchronous background task that listens for and processes incoming
    // witness-related objects (`Message`, `Report`, `AggregatedReport`) for each round.
    //
    // # Returns:
    // * `JoinHandle<()>` — representing the spawned asynchronous task that runs indefinitely.

    fn initialize_witness_handle(&mut self) -> JoinHandle<()>{
        println!("initializing aggregated witness handle...");

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
                        } else if let Ok(aggregated_report) = AggregatedReport::read_json(&received_object) {
                            object = ObjectContent::AggregatedReport(aggregated_report);
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

                                    if count.values >= validity_threshold {
                                        Self::update_witnesses(thread_id, count, content);
                                    }
                                    if count.aggregated_witnesses >= validity_threshold {
                                        Self::update_aggregated_witnesses(thread_id, count, content);
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
                            ObjectContent::AggregatedReport(aggregated_report) => {
                                if !content.aggregated_reports.contains(&aggregated_report) {
                                    content.aggregated_reports.push(aggregated_report);
                                    count.aggregated_reports += 1;  
                                    let aggregated_report = content.aggregated_reports.get_mut((count.aggregated_reports - 1) as usize).unwrap(); 
                                    Self::initialize_aggregated_witnesses(thread_id, aggregated_report, &mut content.aggregated_witnesses, count, content.witnesses.clone()); 
                                }
                            },
                            ObjectContent::BarycentricReport(_) => {
                                panic!("Error: received incompatible object type (BarycentricReport) for aggregated witness broadcast");
                            }
                        }

                        if count.values >= validity_threshold && state.report == false {
                            Self::reliable_broadcast_report(thread_id, &thread_signal_channel, content, None, round_number, protocol_information.clone()).await;
                            state.report = true; 
                        }

                        if count.witnesses >= validity_threshold && state.witnesses == false {
                            if protocol_information == "witness"{
                                let protocol_information = String::from("witness");
                                let instance_number = 0; 
                                let values = Report::new(ReportType::Witness, protocol_information, thread_id, content.values.clone(), None, instance_number, round_number); 
                                thread_channel.send_values(thread_id, values).await;
                                state.witnesses = true; 
                            } else {
                                Self::reliable_broadcast_aggregated_report(thread_id, &thread_signal_channel, content, round_number).await;
                                state.witnesses = true; 
                            }
                        }

                        if count.aggregated_witnesses >= validity_threshold && state.aggregated_witnesses == false {
                            let protocol_information = String::from("aggregated witness");
                            let instance_number = 0; 
                            let values = Report::new(ReportType::Witness, protocol_information, thread_id, content.values.clone(), None, instance_number, round_number); 
                            thread_channel.send_values(thread_id, values).await;
                            state.aggregated_witnesses = true; 
                        }
                    }
                }
            }
        });
        handle
    } 


    // # Method Description: 
    // AThis method asynchronously broadcasts a `Report` containing collected values for a witness round
    // to all participants using the reliable broadcast protocol.
    //
    // # Parameters:
    // * `thread_id` — The ID of the thread performing the broadcast.
    // * `thread_signal_channel` — The `SignalChannels` instance used to disseminate the report.
    // * `content` — The `WitnessRoundContent` containing the collected values.
    // * `round_number` — The current round number of the witness protocol.
    // * `protocol_information` — String describing the active protocol type.
    //
    // # Returns:
    // * `Future<()>` — resolves once the broadcast has been sent.
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
impl<T> ReliableCommunication<T> for AggregatedWitnessCommunicator<T>
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
    // This method spawns an asynchronous background task that manages the Reliable Broadcast protocol.
    // It listens for signals (`Input`, `Echo`, `Vote`) on the reliable handle channel and
    // enforces the reliable broadcast thresholds to ensure consistent message delivery.
    //
    // # Returns:
    // * `JoinHandle<()>` — representing the spawned asynchronous task running the reliable broadcast.
//
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
    // This method processes an `Input` signal in the reliable broadcast protocol,
    // converts the input into an `Echo` signal and broadcasts it.
    //
    // # Parameters:
    // * `_thread_id` — The current thread’s ID (unused here).
    // * `thread_signal_channel` — Signal channel used to broadcast the echo.
    // * `signal` — The input signal to be echoed.
    //
    // # Returns:
    // * `Future<()>` — resolves when the echo broadcast is complete.
    async fn upon_input(_thread_id: u32, thread_signal_channel: &SignalChannels<T>, signal: Signal<T>) {
        let echo = Signal::new(SignalType::Echo, signal.get_content().clone(), signal.get_instance_number(), signal.get_round_number());
        thread_signal_channel.broadcast_signal(echo).await;
    }

    // # Method Description: 
    // This method processes an `Echo` signal in the reliable broadcast protocol,
    // converts the echo into a `Vote` signal and broadcasts it.
    //
    // # Parameters:
    // * `_thread_id` — The current thread’s ID.
    // * `thread_signal_channel` — Signal channel used to broadcast the vote.
    // * `signal` — The echo signal to be converted.
    //
    // # Returns:
    // * `Future<()>` — resolves when the vote broadcast is complete.
    async fn upon_echo(_thread_id: u32, thread_signal_channel: &SignalChannels<T>, signal: Signal<T>) {
        let vote = Signal::new(SignalType::Vote, signal.get_content().clone(), signal.get_instance_number(), signal.get_round_number());
        thread_signal_channel.broadcast_signal(vote).await; 
    }
 
    // # Method Description: 
    // This method processes a `Vote` signal in the reliable broadcast protocol.
    // Once validity is reached, delivers the object contained in the signal
    // to the appropriate delivery channel (`MessageChannels` or `ReportChannels`).
    //
    // # Parameters:
    // * `thread_id` — The ID of the thread processing the vote.
    // * `channel` — The channel type where the object may be delivered to `MessageChannels`or `ReportChannels`.
    // * `signal` — The vote signal containing the object to deliver.
    //
    // # Returns:
    // * `Future<()>` — resolves when the object has been delivered to the correct channel.
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
                        println!("sent: {:?}", &message.get_message());
                        report_channel.send_message(thread_id, message).await;     
                    }
                    ObjectContent::Report(report) => {
                        report_channel.send_report(thread_id, report).await;
                    }
                    ObjectContent::AggregatedReport(aggregated_report) => {
                        report_channel.send_aggregated_report(thread_id, aggregated_report).await;
                    },
                    ObjectContent::BarycentricReport(_) => {
                        panic!("Error: received incompatible object type (BarycentricReport) for aggregated witness broadcast");
                    }
                }
            },
        }
    }
}

impl<T> BasicCommunication<T> for AggregatedWitnessCommunicator<T>
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
// This struct represents a collection of individual `Report` objects combined
// into a single higher-level structure, used in witness-based reliable communication
// protocols for consensus and verification.
//
// # Fields:
// * report_type - Defines the type of this aggregated report, such as `Report` or `Witness`.
// * protocol_information - String metadata describing the protocol associated with this aggregated report.
// * id - The identifier of the node or thread that created this aggregated report.
// * reports - A vector of `Report` objects that were collected and combined.
// * instance_number - The instance of the protocol execution this aggregated report belongs to.
// * round_number - The communication round within the broadcast protocol to maintain ordering and separation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregatedReport<T>{
    report_type: ReportType,
    protocol_information: String, 
    id: u32, 
    reports: Vec<Report<T>>, 
    instance_number: u32,
    round_number: u32
}

impl<T> AggregatedReport<T>
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

    pub fn get_reports(&self) -> &Vec<Report<T>> {
        &self.reports
    }

    pub fn get_instance_number(&self) -> u32 {
        self.instance_number
    }

    pub fn get_round_number(&self) -> u32 {
        self.round_number
    }

    pub fn new(report_type: ReportType, protocol_information: String, id: u32, reports: Vec<Report<T>>, instance_number: u32, round_number: u32) -> Self {
        Self {
            report_type,
            protocol_information,
            id, 
            reports,
            instance_number,
            round_number
        }
    }
}

impl<T> JsonConversion<AggregatedReport<T>> for AggregatedReport<T>
where 
    T: Debug + Clone + Serialize +  DeserializeOwned + PartialEq + Eq + Hash,
{}