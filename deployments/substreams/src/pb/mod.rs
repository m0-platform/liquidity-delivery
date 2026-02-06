// @generated
// @@protoc_insertion_point(attribute:schema)
pub mod schema {
    include!("schema.rs");
    // @@protoc_insertion_point(schema)
}
pub mod sf {
    pub mod ethereum {
        pub mod r#type {
            // @@protoc_insertion_point(attribute:sf.ethereum.type.v2)
            pub mod v2 {
                include!("sf.ethereum.type.v2.rs");
                // @@protoc_insertion_point(sf.ethereum.type.v2)
            }
        }
        pub mod substreams {
            // @@protoc_insertion_point(attribute:sf.ethereum.substreams.v1)
            pub mod v1 {
                include!("sf.ethereum.substreams.v1.rs");
                // @@protoc_insertion_point(sf.ethereum.substreams.v1)
            }
        }
        pub mod transform {
            // @@protoc_insertion_point(attribute:sf.ethereum.transform.v1)
            pub mod v1 {
                include!("sf.ethereum.transform.v1.rs");
                // @@protoc_insertion_point(sf.ethereum.transform.v1)
            }
        }
    }
}
pub mod substreams {
    pub mod v1 {
        // @@protoc_insertion_point(attribute:substreams.v1.program)
        pub mod program {
            include!("substreams.v1.program.rs");
            // @@protoc_insertion_point(substreams.v1.program)
        }
    }
}
