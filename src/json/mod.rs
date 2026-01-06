use serde::{Deserialize, Serialize};

// # Trait Description:
// This trait provides a unified interface for serializing and deserializing types
// to and from JSON. It can be implemented by any type that supports Serde's
// `Serialize` and `Deserialize` traits.
//
// # Type Parameters:
// * T - The concrete type that implements both `Serialize` and `Deserialize`,
//        enabling JSON conversion.
pub trait JsonConversion<T>: Serialize
where 
    T: Serialize + for<'de> Deserialize<'de>
{

    // # Method Description
    // Constructs a new instance of type `T` from a JSON string.
    // # Parameters:
    // * data - A reference to a JSON-formatted `String`.
    // # Returns:
    // * `Ok(T)` if deserialization succeeds, otherwise a `serde_json::Error`.
    fn read_json(data:& String) -> Result<T, serde_json::Error> {
        serde_json::from_str(data)
    }    
    // # Method Description
    // Converts the struct instance into a JSON string.
    // # Returns:
    // * A `String` containing the JSON representation of the instance.
    fn write_json(&self) -> String {
        serde_json::to_string(self).expect("Error: JSON object could not be created")
    }
}
