#![allow(unused_imports)]

use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use automerge::{ActorId, AutoCommit, Automerge, ObjType, ReadDoc};
use automerge::transaction::Transactable;
use criterion::{black_box, Criterion};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use smartstring::alias::String as SmartString;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditHistory {
    start_content: SmartString,
    end_content: String,

    txns: Vec<HistoryEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimpleTextOp(usize, usize, SmartString); // pos, del_len, ins_content.

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    id: usize,
    parents: SmallVec<[usize; 2]>,
    num_children: usize,
    agent: String,
    // op: TextOperation,
    ops: SmallVec<[SimpleTextOp; 2]>,
}


fn gen_main() -> Result<(), Box<dyn Error>> {
    let mut doc = AutoCommit::new();
    let text_id = doc.put_object(automerge::ROOT, "text", ObjType::Text)?;

    // doc.splice_text(id, 0, 0, "hi there")?;
    // dbg!(&doc.get_heads());

    // let filename = "example_trace.json";
    let filename = "node_nodecc.json";
    // let filename = "git_makefile.json";

    let file = BufReader::new(File::open(filename)?);
    let history: EditHistory = serde_json::from_reader(file)?;
    // dbg!(data);

    assert!(history.start_content.is_empty()); // 'cos I'm not handling this for now.

    // There should be exactly one entry with no parents.
    let num_roots = history.txns.iter().filter(|e| e.parents.is_empty()).count();
    // assert_eq!(num_roots, 1);

    // The last item should be the output.
    let num_final = history.txns.iter().filter(|e| e.num_children == 0).count();
    assert_eq!(num_final, 1);

    let mut doc_at_idx: HashMap<usize, (AutoCommit, usize)> = HashMap::new();
    doc_at_idx.insert(usize::MAX, (doc, num_roots));

    fn take_doc(doc_at_idx: &mut HashMap<usize, (AutoCommit, usize)>, idx: usize) -> AutoCommit {
        let (parent_doc, retains) = doc_at_idx.get_mut(&idx).unwrap();
        if *retains == 1 {
            // We'll just take the document.
            doc_at_idx.remove(&idx).unwrap().0
        } else {
            // Fork it and take the fork.
            *retains -= 1;
            parent_doc.fork()
        }
    }

    // doc_at_idx.insert(usize::MAX)

    // let mut root = Some(doc);
    for entry in history.txns.iter() {
        // First we need to get the doc we're editing.
        let (&first_p, rest_p) = entry.parents.split_first().unwrap_or((&usize::MAX, &[]));

        let mut doc = take_doc(&mut doc_at_idx, first_p);

        // If there's any more parents, merge them together.
        for p in rest_p {
            let mut doc2 = take_doc(&mut doc_at_idx, *p);
            doc.merge(&mut doc2).unwrap();
        }

        // Gross - actor IDs are fixed 16 byte arrays.
        // let actor = ActorId::from()
        // let mut actor_bytes = [0u8; 16];
        // let copied_bytes = actor_bytes.len().min(entry.agent.len());
        // actor_bytes[..copied_bytes].copy_from_slice(&entry.agent.as_bytes()[..copied_bytes]);

        // This is necessary or we get duplicate actor/seq pairs. It should be possible to just keep
        // using the same actor with new sequence numbers for subsequent changes, but I don't think
        // the automerge API makes this possible.
        // actor_bytes[12..16].copy_from_slice(&(entry.id as u32).to_be_bytes());
        // let actor = ActorId::from(actor_bytes);
        // doc.set_actor(actor);


        // Ok, now modify the document.
        for op in &entry.ops {
            doc.splice_text(text_id.clone(), op.0, op.1, &op.2).unwrap();
        }

        doc.commit();

        // And deposit the result back into doc_at_idx.
        if entry.num_children > 0 {
            doc_at_idx.insert(entry.id, (doc, entry.num_children));
        } else {
            println!("done!");
            let result = doc.text(text_id.clone()).unwrap();
            // println!("result: '{result}'");
            let saved = doc.save();
            println!("automerge document saves to {} bytes", saved.len());

            let out_filename = format!("{filename}.am");
            std::fs::write(&out_filename, saved).unwrap();
            println!("Saved to {out_filename}");

            assert_eq!(result, history.end_content);
        }
    }

    Ok(())
}

fn bench_process(c: &mut Criterion) {
    let name = "node_nodecc";
    let filename = format!("{name}.json.am");

    c.bench_function(&format!("process_remote_edits/{name}"), |b| {
        let bytes = std::fs::read(&filename).unwrap();
        b.iter(|| {
            let doc = AutoCommit::load(&bytes).unwrap();
            black_box(doc);
            // let (_, text_id) = doc.get(automerge::ROOT, "text").unwrap().unwrap();
            // let result = doc.text(text_id).unwrap();
            // black_box(result);
        })
    });
}

fn bench_main() {
    // benches();
    let mut c = Criterion::default()
        .configure_from_args();

    bench_process(&mut c);
    c.final_summary();
}

fn main() {
    gen_main().unwrap();
    // bench_main();
}