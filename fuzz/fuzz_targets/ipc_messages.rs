#![no_main]

use libfuzzer_sys::fuzz_target;
use omg_lib::daemon::protocol::{Request, Response};

fn response_id(response: &Response) -> u64 {
    match response {
        Response::Success { id, .. } | Response::Error { id, .. } => *id,
    }
}

fuzz_target!(|data: &[u8]| {
    if let Ok(request) = bitcode::deserialize::<Request>(data) {
        let encoded = bitcode::serialize(&request)
            .expect("invariant: a successfully decoded request must be encodable");
        let decoded = bitcode::deserialize::<Request>(&encoded)
            .expect("invariant: an encoded request must be decodable");

        assert_eq!(request.id(), decoded.id());
        assert_eq!(
            encoded,
            bitcode::serialize(&decoded)
                .expect("invariant: a round-tripped request must remain encodable")
        );
    }

    if let Ok(response) = bitcode::deserialize::<Response>(data) {
        let encoded = bitcode::serialize(&response)
            .expect("invariant: a successfully decoded response must be encodable");
        let decoded = bitcode::deserialize::<Response>(&encoded)
            .expect("invariant: an encoded response must be decodable");

        assert_eq!(response_id(&response), response_id(&decoded));
        assert_eq!(
            encoded,
            bitcode::serialize(&decoded)
                .expect("invariant: a round-tripped response must remain encodable")
        );
    }
});
