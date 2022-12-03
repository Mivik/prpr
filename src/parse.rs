mod pgr;
pub use pgr::parse_phigros;

mod rpe;
pub use rpe::parse_rpe;

use crate::ext::NotNanExt;

fn process_lines(v: &mut [crate::core::JudgeLine]) {
    let mut times = Vec::new();
    let mut process_notes = |v: &mut [crate::core::Note]| {
        let mut i = 0;
        while i < v.len() {
            times.push(v[i].time.not_nan());
            let mut j = i + 1;
            // since times are generated in the same way, theoretically we can compare them directly
            while j < v.len() && v[j].time == v[i].time {
                j += 1;
            }
            v[i..j].sort_by_key(|it| -it.kind.order());
            if j != i + 1 {
                times.push(v[i].time.not_nan());
            }
            i = j;
        }
    };
    for line in v.iter_mut() {
        process_notes(&mut line.notes_above);
        process_notes(&mut line.notes_below);
    }
    times.sort();
    let mut mt = Vec::new();
    for i in 0..(times.len() - 1) {
        if times[i] == times[i + 1] && (i == 0 || times[i - 1] != times[i]) {
            mt.push(*times[i]);
        }
    }
    let process_notes = |v: &mut [crate::core::Note]| {
        let mut i = 0;
        for note in v.iter_mut() {
            let time = note.time;
            while i < mt.len() && mt[i] < time {
                i += 1;
            }
            if mt[i] == time {
                note.multiple_hint = true;
            }
        }
    };
    for line in v.iter_mut() {
        process_notes(&mut line.notes_above);
        process_notes(&mut line.notes_below);
    }
}
