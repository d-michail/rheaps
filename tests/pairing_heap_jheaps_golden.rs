use core::fmt::Write;

use rheaps::tree::{PairingHeap, TreeHandle};

fn replay(script: &str) -> String {
    let mut heap = PairingHeap::<i32, usize>::new();
    let mut handles: Vec<Option<TreeHandle>> = Vec::new();
    let mut output = String::new();

    for line in script.lines() {
        let mut fields = line.split_whitespace();
        match fields.next() {
            Some("I") => {
                let key = fields.next().unwrap().parse().unwrap();
                let id = fields.next().unwrap().parse::<usize>().unwrap();
                assert_eq!(id, handles.len());
                handles.push(Some(heap.insert(key, id)));
            }
            Some("P") => {
                let (key, id) = heap
                    .pop_entry()
                    .expect("golden script popped an empty heap");
                handles[id] = None;
                writeln!(output, "P {key} {id}").unwrap();
            }
            Some("D") => {
                let id = fields.next().unwrap().parse::<usize>().unwrap();
                let key = fields.next().unwrap().parse().unwrap();
                heap.decrease_key(handles[id].unwrap(), key).unwrap();
            }
            Some("X") => {
                let id = fields.next().unwrap().parse::<usize>().unwrap();
                let (key, value) = heap.delete(handles[id].take().unwrap()).unwrap();
                writeln!(output, "X {key} {value}").unwrap();
            }
            operation => panic!("unknown golden operation: {operation:?}"),
        }
        assert!(fields.next().is_none(), "trailing fields in golden script");
    }

    output
}

macro_rules! golden_test {
    ($name:ident, $fixture:literal) => {
        #[test]
        fn $name() {
            let script = include_str!(concat!("data/", $fixture, ".ops"));
            let expected = include_str!(concat!("data/", $fixture, ".out"));
            assert_eq!(replay(script), expected);
        }
    };
}

golden_test!(
    decrease_key_preserves_jheaps_tie_order,
    "pairing_heap_decrease"
);
golden_test!(delete_preserves_jheaps_tie_order, "pairing_heap_delete");
golden_test!(mixed_cuts_preserve_jheaps_tie_order, "pairing_heap_mixed");
