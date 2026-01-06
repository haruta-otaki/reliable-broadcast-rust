// # Program Description: 
// This program simulates inter-thread communication using tokio's async message passing feature.
// # Author: Haruta Otaki
// # Date: June 19th, 2025

use std::{env}; 
use rust_project::aggregated_witness::{AggregatedWitnessCommunication, AggregatedWitnessCommunicator, AggregatedWitnessHub};
use rust_project::barycentric_agreement::{BarycentricCommunication, BarycentricCommunicator, BarycentricHub};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::{task::JoinHandle};
use rust_project::basic::{BasicCommunication, BasicHub, BasicCommunicator};
use rust_project::reliable::{ReliableCommunication, ReliableHub, ReliableCommunicator};
use rust_project::witness::{WitnessCommunication, WitnessHub, WitnessCommunicator};

// # Function Description: 
// This function creates a set of asynchronous channels for inter-thread communication.
// # Parameters:
// * thread_count - total number of threads in the simulation
// # Returns
// * a vector of sending handles per thread
//  * a vector of receiving handles per thread
fn create_channels(thread_count: u32) -> (Vec<Sender<String>>, Vec<Receiver<String>> ) {
    let mut receivers: Vec<Receiver<String>> = vec![];
    let mut transmitters: Vec<Sender<String>> = vec![];

    for _ in 0..thread_count{
        // adjust the buffer size according to the number of threads participating 
        let (tx, rx) = mpsc::channel(256); 
        transmitters.push(tx);
        receivers.push(rx);
    }
    (transmitters, receivers)
}

// # Function Description: 
// This function spawns an asynchronous thread simulating a node in a witness-based reliable broadcast network. 
// The thread executes a predefined sequence of witness and reliable communication actions,
// primarily intended for testing and validating communication protocols between nodes.
//
// # Parameters:
// * id - the unique identifier for this thread, used to determine which communication actions it performs.
// * witness_communicator - a `WitnessCommunicator` instance, encapsulating communication logic 
//   for both witness-based and reliable broadcast protocols as well as basic message passing.
//
// # Returns:
// * a `JoinHandle<()>` representing the spawned asynchronous task.
fn create_witness_thread (id: u32, mut witness_communicator: WitnessCommunicator<String>) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let reliable_handle = witness_communicator.initialize_reliable_handle(); 
            let witness_handle = witness_communicator.initialize_witness_handle(); 
            
            println!("Testing... Round 1, witness communication"); 
            if id == 0 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                witness_communicator.witness_broadcast(message, 0).await; 
                
            }

            if id == 1 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                witness_communicator.witness_broadcast(message, 0).await; 
                
            }

            if id == 2 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                witness_communicator.witness_broadcast(message, 0).await; 
                
            }

            if id == 3 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                witness_communicator.witness_broadcast(message, 0).await; 
                
            }

            if id == 4 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                witness_communicator.witness_broadcast(message, 0).await; 
                
            }

            if id == 5 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                witness_communicator.witness_broadcast(message, 0).await; 
                
            }
          
            println!("id: {id}, collecting...");
            witness_communicator.witness_collect(0).await; 

            println!("Testing... Round 2, witness communication"); 
            if id == 0 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                witness_communicator.witness_broadcast(message, 1).await; 
                
            }

            if id == 1 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                witness_communicator.witness_broadcast(message, 1).await; 
                
            }

            if id == 2 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                witness_communicator.witness_broadcast(message, 1).await; 
                
            }

            if id == 3 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                witness_communicator.witness_broadcast(message, 1).await; 
                
            }

            if id == 4 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                witness_communicator.witness_broadcast(message, 1).await; 
                
            }

            if id == 5 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                witness_communicator.witness_broadcast(message, 1).await; 
                
            }
          
            println!("id: {id}, collecting...");
            witness_communicator.witness_collect(1).await; 

            //test reliable broadcast           
            if id == 0 {
                println!("Testing... Round 3, reliable communication"); 
                println!("id: {id}, reliable broadcasting..."); 
                let message = format!("reliable broadcast message by {id}");
                witness_communicator.reliable_broadcast(message, 0, 2).await; 
            }

            println!("id: {id}, reliable receiving...");
            witness_communicator.reliable_recv(Some(0), 0, 2).await; 

             //test send() & recv()
             if id == 2 {
                println!("Testing... Round 3, basic communication"); 
                println!("id: {id}, sending..."); 
                let message = format!("message from {} to {}", id, 1);
                witness_communicator.basic_send(1, message, 2).await; 
            }

            if id == 1 {
                println!("id: {id}, receiving...");
                witness_communicator.basic_recv(Some(2), 2).await; 
            }

            witness_communicator.terminate_reliable_handle(reliable_handle);
            witness_communicator.terminate_witness_handle(witness_handle);

            println!("id: {id}, break");
            break; 
        }
    })
}

// # Function Description
// This function spawns an asynchronous task representing a node participating in a 
// barycentric agreement and reliable broadcast protocol. 
//
// # Parameters
// * id - a unique identifier for the thread, determining which specific actions the node 
//   performs during each communication round. Each ID corresponds to a distinct participant.
//
// * barycentric_communicator - a `BarycentricCommunicator` instance encapsulating communication 
//   logic for Reliable Broadcast Protocol.
//
// # Returns
// * `JoinHandle<()>` - a handle to the asynchronously spawned Tokio task representing this node.
fn create_barycentric_agreement_thread (id: u32, mut barycentric_communicator: BarycentricCommunicator<String>) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let reliable_handle = barycentric_communicator.initialize_reliable_handle(); 
            let barycentric_handle = barycentric_communicator.initialize_barycentric_handle(); 
            
            println!("Testing... Round 1, barycentric agreement"); 
            if id == 0 {
                println!("id: {id}, barycentric agreement..."); 
                let message = format!("barycentric agreement broadcast message by {id}");
                barycentric_communicator.barycentric_agreement(message, 0).await; 
                
            }

            if id == 1 {
                println!("id: {id}, barycentric agreement..."); 
                let message = format!("barycentric agreement broadcast message by {id}");
                barycentric_communicator.barycentric_agreement(message,  0).await; 
                
            }

            if id == 2 {
                println!("id: {id}, barycentric agreement...");
                let message = format!("barycentric agreement broadcast message by {id}");
                barycentric_communicator.barycentric_agreement(message, 0).await; 
                
            }

             if id == 3 {
                println!("id: {id}, barycentric agreement...");
                let message = format!("barycentric agreement broadcast message by {id}");
                barycentric_communicator.barycentric_agreement(message, 0).await; 
                
            }

             if id == 4 {
                println!("id: {id}, barycentric agreement...");
                let message = format!("barycentric agreement broadcast message by {id}");
                barycentric_communicator.barycentric_agreement(message, 0).await; 
                
            }

             if id == 5 {
                println!("id: {id}, barycentric agreement...");
                let message = format!("barycentric agreement broadcast message by {id}");
                barycentric_communicator.barycentric_agreement(message, 0).await; 
                
            }
          
            println!("id: {id}, collecting...");
            barycentric_communicator.barycentric_collect(0).await; 

          
            println!("Testing... Round 2, barycentric agreement"); 
            if id == 0 {
                println!("id: {id}, barycentric agreement..."); 
                let message = format!("barycentric agreement broadcast message by {id}");
                barycentric_communicator.barycentric_agreement(message, 1).await; 
                
            }

            if id == 1 {
                println!("id: {id}, barycentric agreement..."); 
                let message = format!("barycentric agreement broadcast message by {id}");
                barycentric_communicator.barycentric_agreement(message, 1).await; 
                
            }

            if id == 2 {
                println!("id: {id}, barycentric agreement...");
                let message = format!("barycentric agreement broadcast message by {id}");
                barycentric_communicator.barycentric_agreement(message, 1).await; 
                
            }

             if id == 3 {
                println!("id: {id}, barycentric agreement...");
                let message = format!("barycentric agreement broadcast message by {id}");
                barycentric_communicator.barycentric_agreement(message, 1).await; 
                
            }

             if id == 4 {
                println!("id: {id}, barycentric agreement...");
                let message = format!("barycentric agreement broadcast message by {id}");
                barycentric_communicator.barycentric_agreement(message, 1).await; 
                
            }

             if id == 5 {
                println!("id: {id}, barycentric agreement...");
                let message = format!("barycentric agreement broadcast message by {id}");
                barycentric_communicator.barycentric_agreement(message, 1).await; 
                
            }
          
            println!("id: {id}, collecting...");
            barycentric_communicator.barycentric_collect(1).await; 

            //test reliable broadcast           
            if id == 0 {
                println!("Testing... Round 3, reliable communication"); 
                println!("id: {id}, reliable broadcasting..."); 
                let message = format!("reliable broadcast message by {id}");
                barycentric_communicator.reliable_broadcast(message, 0, 2).await; 
            }

            println!("id: {id}, reliable receiving...");
            barycentric_communicator.reliable_recv(Some(0), 0, 2).await; 

             //test send() & recv()
             if id == 2 {
                println!("Testing... Round 3, basic communication"); 
                println!("id: {id}, sending..."); 
                let message = format!("message from {} to {}", id, 1);
                barycentric_communicator.basic_send(1, message, 2).await; 
            }

            if id == 1 {
                println!("id: {id}, receiving...");
                barycentric_communicator.basic_recv(Some(2), 2).await; 
            }

            barycentric_communicator.terminate_reliable_handle(reliable_handle);
            barycentric_communicator.terminate_barycentric_handle(barycentric_handle);

            println!("id: {id}, break");
            break; 
        }
    })
}


// # Function Description: 
// This function spawns a single asynchronous thread simulating a node in a reliable broadcast network. 
//The thread executes a predefined sequence of reliable communication actions for testing purposes.
// # Parameters:
// * id - the unique identifier for this thread.
// * reliable_communicator - a `ReliableCommunicator` instance, encapsulating communication logic for this thread.
// # Returns:
// * a `JoinHandle<()>` representing the asynchronous task.
fn create_reliable_thread (id:u32, mut reliable_communicator: ReliableCommunicator<String>) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let reliable_handle = reliable_communicator.initialize_reliable_handle(); 
            //reliable broadcast testing            
            if id == 0 {
                println!("Testing... Round 1, reliable communication"); 
                println!("id: {id}, reliable broadcasting..."); 
                let message = format!("reliable broadcast message by {id}");
                reliable_communicator.reliable_broadcast(message, 0, 0).await; 
            }

            //test: create f faulty thread
            // if id == 1 && count == 0{
            //     println!("id: {id}, shutdown...");
            //     thread.recv(None).await; 
            // }

            println!("id: {id}, reliable receiving...");
            reliable_communicator.reliable_recv(Some(0), 0, 0).await; 
            
            if id == 1 {
                println!("Testing... Round 2, reliable communication"); 
                println!("id: {id}, reliable broadcasting..."); 
                let message = format!("reliable broadcast message by {id}");
                reliable_communicator.reliable_broadcast(message, 1, 0).await; 
            }

            // test: multiple reliable_broadcast calls
            println!("id: {id}, reliable receiving...");
            reliable_communicator.reliable_recv(Some(1),1, 0).await; 
            
            //test send() & recv()
            if id == 2 {
                println!("Testing... Round 3, basic communication"); 
                println!("id: {id}, sending..."); 
                let message = format!("message from {} to {}", id, 1);
                reliable_communicator.basic_send(1, message, 0).await; 
            }

            if id == 1 {
                println!("id: {id}, receiving...");
                reliable_communicator.basic_recv(Some(2), 0).await; 
            }

            reliable_communicator.terminate_reliable_handle(reliable_handle);
            println!("id: {id}, break");
            break; 
        }
    })
}
// # Function Description: 
// This function spawns a single asynchronous thread simulating a node in a basic message-passing network. 
// The thread executes a predefined sequence of basic communication actions for testing purposes.
// # Parameters:
// * id - the unique identifier for this thread.
// * basic_communicator - a `BasicCommunicator` instance, encapsulating basic send, recv, and broadcast logic.
// # Returns:
// * a `JoinHandle<()>` representing the asynchronous task.
fn create_basic_thread (id:u32, mut basic_communicator: BasicCommunicator<String>) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            //basic testing
            if id == 0 {
                let message = format!("message from {} to {}", id, 1);
                println!("id: {id}, sending..."); 
                basic_communicator.basic_send(1, message, 0).await;
            }
            if id == 1 {
                let message = format!("message from {} to {}", id, 2);
                println!("id: {id}, sending..."); 
                basic_communicator.basic_send(2, message, 0).await;
            }
            if id == 1 {
                println!("id: {id}, receiving..."); 
                basic_communicator.basic_recv(None, 0).await; 
            }
            if id == 2 {
                println!("id: {id}, receiving..."); 
                basic_communicator.basic_recv(Some(1), 0).await; 
            }
            if id == 0 {
                println!("id: {id}, broadcasting..."); 
                let message = format!("broadcast message from {id}");
                basic_communicator.basic_broadcast(message, 0).await;
            }

            println!("id: {id}, receiving..."); 
            basic_communicator.basic_recv(Some(0), 0).await; 

            println!("id: {id}, break");
            break; 
            
        }
    })
}

// # Function Description
// This function spawns an asynchronous task that simulates a node participating in an 
// aggregated witness-based reliable broadcast network.
//
// # Parameters
// * id - a unique numeric identifier for this node/thread.  
// * aggregated_witness_communicator - an instance of `AggregatedWitnessCommunicator`, which 
//   encapsulates all the underlying communication protocols, including standard Witness Broadcast Protocol,
//   Reliable Broadcast Protocol
//
// # Returns
// * `JoinHandle<()>` - A handle to the asynchronous Tokio task representing this node’s execution.
fn create_aggregated_witness_thread (id: u32, mut aggregated_witness_communicator: AggregatedWitnessCommunicator<String>) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let reliable_handle = aggregated_witness_communicator.initialize_reliable_handle(); 
            let witness_handle = aggregated_witness_communicator.initialize_witness_handle(); 
            
            println!("Testing... Round 1, aggregated witness communication"); 
            if id == 0 {
                println!("id: {id}, aggregated witness broadcasting..."); 
                let message = format!("aggregated witness broadcast message by {id}");
                aggregated_witness_communicator.aggregated_witness_broadcast(message, 0).await; 
                
            }

            if id == 1 {
                println!("id: {id}, aggregated witness broadcasting..."); 
                let message = format!("aggregated witness broadcast message by {id}");
                aggregated_witness_communicator.aggregated_witness_broadcast(message, 0).await; 
                
            }

            if id == 2 {
                println!("id: {id}, aggregated witness broadcasting..."); 
                let message = format!("aggregated witness broadcast message by {id}");
                aggregated_witness_communicator.aggregated_witness_broadcast(message, 0).await; 
                
            }

            if id == 3 {
                println!("id: {id}, aggregated witness broadcasting..."); 
                let message = format!("aggregated witness broadcast message by {id}");
                aggregated_witness_communicator.aggregated_witness_broadcast(message, 0).await; 
                
            }

            if id == 4 {
                println!("id: {id}, aggregated witness broadcasting..."); 
                let message = format!("aggregated witness broadcast message by {id}");
                aggregated_witness_communicator.aggregated_witness_broadcast(message, 0).await; 
                
            }

            if id == 5 {
                println!("id: {id}, aggregated witness broadcasting..."); 
                let message = format!("aggregated witness broadcast message by {id}");
                aggregated_witness_communicator.aggregated_witness_broadcast(message, 0).await; 
                
            }
          
            println!("id: {id}, aggregated collecting...");
            aggregated_witness_communicator.aggregated_witness_collect(0).await; 


            println!("Testing... Round 2, aggregated witness communication"); 
            if id == 0 {
                println!("id: {id}, aggregated witness broadcasting..."); 
                let message = format!("aggregated witness broadcast message by {id}");
                aggregated_witness_communicator.aggregated_witness_broadcast(message, 1).await; 
                
            }

            if id == 1 {
                println!("id: {id}, aggregated witness broadcasting..."); 
                let message = format!("aggregated witness broadcast message by {id}");
                aggregated_witness_communicator.aggregated_witness_broadcast(message, 1).await; 
                
            }

            if id == 2 {
                println!("id: {id}, aggregated witness broadcasting..."); 
                let message = format!("aggregated witness broadcast message by {id}");
                aggregated_witness_communicator.aggregated_witness_broadcast(message, 1).await; 
                
            }

            if id == 3 {
                println!("id: {id}, aggregated witness broadcasting..."); 
                let message = format!("aggregated witness broadcast message by {id}");
                aggregated_witness_communicator.aggregated_witness_broadcast(message, 1).await; 
                
            }

            if id == 4 {
                println!("id: {id}, aggregated witness broadcasting..."); 
                let message = format!("aggregated witness broadcast message by {id}");
                aggregated_witness_communicator.aggregated_witness_broadcast(message, 1).await; 
                
            }

            if id == 5 {
                println!("id: {id}, aggregated witness broadcasting..."); 
                let message = format!("aggregated witness broadcast message by {id}");
                aggregated_witness_communicator.aggregated_witness_broadcast(message, 1).await; 
                
            }
          
            println!("id: {id}, aggregated collecting...");
            aggregated_witness_communicator.aggregated_witness_collect(1).await; 

            println!("Testing... Round 3, aggregated witness communication"); 
            if id == 0 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                aggregated_witness_communicator.witness_broadcast(message, 2).await; 
                
            }

            if id == 1 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                aggregated_witness_communicator.witness_broadcast(message, 2).await; 
                
            }

            if id == 2 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                aggregated_witness_communicator.witness_broadcast(message, 2).await; 
                
            }

            if id == 3 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                aggregated_witness_communicator.witness_broadcast(message, 2).await; 
                
            }

            if id == 4 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                aggregated_witness_communicator.witness_broadcast(message, 2).await; 
                
            }

            if id == 5 {
                println!("id: {id}, witness broadcasting..."); 
                let message = format!("witness broadcast message by {id}");
                aggregated_witness_communicator.witness_broadcast(message, 2).await; 
                
            }

            println!("id: {id}, collecting...");
            aggregated_witness_communicator.witness_collect(2).await; 

            //test reliable broadcast           
            if id == 0 {
                println!("Testing... Round 4, aggregated reliable communication"); 
                println!("id: {id}, reliable broadcasting..."); 
                let message = format!("reliable broadcast message by {id}");
                aggregated_witness_communicator.reliable_broadcast(message, 0, 3).await; 
            }

            println!("id: {id}, reliable receiving...");
            aggregated_witness_communicator.reliable_recv(Some(0), 0, 3).await; 

             //test send() & recv()
             if id == 2 {
                println!("Testing... Round 5, aggregated basic communication"); 
                println!("id: {id}, sending..."); 
                let message = format!("message from {} to {}", id, 1);
                aggregated_witness_communicator.basic_send(1, message, 3).await; 
            }

            if id == 1 {
                println!("id: {id}, receiving...");
                aggregated_witness_communicator.basic_recv(Some(2), 3).await; 
            }

            aggregated_witness_communicator.terminate_reliable_handle(reliable_handle);
            aggregated_witness_communicator.terminate_witness_handle(witness_handle);

            println!("id: {id}, break");
            break; 
        }
    })
}

// # Function Description:
// This asynchronous function sets up and spawns a collection of simulated threads
// for testing different message-passing communication models: either a `BasicHub` or a `ReliableHub`.
// # Parameters:
// * `transmitters` - a vector of `Sender<String>` objects, each representing the outgoing message channel for a thread.
// * `receivers` - a vector of `Receiver<String>` objects, each representing the incoming message channel for a thread.
// * `thread_count` - the number of threads to spawn (and thus the number of communicators to create).
// * `communication_type` - a string reference that specifies the communication mode ("basic" or "reliable").
async fn simulate_threads(transmitters: Vec<Sender<String>>, receivers: Vec<Receiver<String>>,
    thread_count: u32, communication_type: &String) {
    let mut handles = vec![];

    if communication_type == "basic" {
        println!("Setting up basic communication..."); 
        let mut basic_hub = BasicHub::new(transmitters, receivers, thread_count); 
        for i in 0..thread_count {
            let handle: JoinHandle<()> = create_basic_thread(i as u32, basic_hub.create_basic_communicator());
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await.unwrap();
        }
    }
    else if communication_type == "reliable" {
        println!("Setting up reliable communication...");      
        let mut reliable_hub = ReliableHub::new(transmitters, receivers, thread_count);    
        for i in 0..thread_count {
            let handle: JoinHandle<()> = create_reliable_thread(i as u32, reliable_hub.create_reliable_communicator());
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await.unwrap();
        }
    } else if communication_type == "witness" {
        println!("Setting up witness communication...");      
        let mut witness_hub = WitnessHub::new(transmitters, receivers, thread_count);    
        for i in 0..thread_count {
            let handle: JoinHandle<()> = create_witness_thread(i as u32, witness_hub.create_witness_communicator());
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await.unwrap();
        }
    } else if communication_type == "aggregated_witness" {
        println!("Setting up aggregated witness communication...");      
        let mut aggregated_witness_hub = AggregatedWitnessHub::new(transmitters, receivers, thread_count);    
        for i in 0..thread_count {
            let handle: JoinHandle<()> = create_aggregated_witness_thread(i as u32, aggregated_witness_hub.create_aggregated_witness_communicator());
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await.unwrap();
        }
    }  else {
        println!("Setting up barycentric agreement communication...");      
        let mut barycentric_agreement_hub = BarycentricHub::new(transmitters, receivers, thread_count);    
        for i in 0..thread_count {
            let handle: JoinHandle<()> = create_barycentric_agreement_thread(i as u32, barycentric_agreement_hub.create_barycentric_communicator());
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await.unwrap();
        }
    } 
}

#[tokio::main] 
async fn main() {
    //takes in the number of threads to simulate from the command-line argument
    let args: Vec<String> = env::args().collect();
    let thread_count:u32 = args[1].parse().unwrap(); 
    let communication_type: String = args[2].parse().unwrap(); 
    
    let (transmitters, receivers) = create_channels(thread_count);
    simulate_threads(transmitters, receivers, thread_count, &communication_type).await;
}