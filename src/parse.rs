mod pec;
pub use pec::parse_pec;

mod pgr;
pub use pgr::parse_phigros;

mod rpe;
pub use rpe::parse_rpe;

fn process_lines(v: &mut [crate::core::JudgeLine]) {
    use crate::ext::NotNanExt;
    let mut times = Vec::new();
    // TODO optimize using k-merge sort
    let mut process_notes = |v: &mut [crate::core::Note]| {
        v.sort_by_key(|it| it.time.not_nan());
        let mut i = 0;
        while i < v.len() {
            times.push(v[i].time.not_nan());
            let mut j = i + 1;
            while j < v.len() && v[j].time == v[i].time {
                j += 1;
            }
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
        // since times are generated in the same way, theoretically we can compare them directly
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
            if i < mt.len() && mt[i] == time {
                note.multiple_hint = true;
            }
        }
        v.sort_by_key(|it| it.kind.order());
    };
    for line in v.iter_mut() {
        process_notes(&mut line.notes_above);
        process_notes(&mut line.notes_below);
    }
}

#[rustfmt::skip]
const TWEEN_MAP: [crate::core::TweenId; 30] = {
    use crate::core::{easing_from as e, TweenMajor::*, TweenMinor::*};
    [
        2, 2, // linear
        e(Sine, Out), e(Sine, In),
        e(Quad, Out), e(Quad, In),
        e(Sine, InOut), e(Quad, InOut),
        e(Cubic, Out), e(Cubic, In),
        e(Quart, Out), e(Quart, In),
        e(Cubic, InOut), e(Quart, InOut),
        e(Quint, Out), e(Quint, In),
        e(Expo, Out), e(Expo, In),
        e(Circ, Out), e(Circ, In),
        e(Back, Out), e(Back, In),
        e(Circ, InOut), e(Back, InOut),
        e(Elastic, Out), e(Elastic, In),
        e(Bounce, Out), e(Bounce, In),
        e(Bounce, InOut), e(Elastic, InOut),
    ]
};

#[derive(serde::Deserialize)]
struct Triple(u32, u32, u32);

impl Triple {
    pub fn beats(&self) -> f32 {
        self.0 as f32 + self.1 as f32 / self.2 as f32
    }
}

struct BpmList {
    elements: Vec<(f32, f32, f32)>, // (beats, time, bpm)
    cursor: usize,
}

impl BpmList {
    pub fn new(ranges: Vec<(f32, f32)>) -> Self {
        let mut elements = Vec::new();
        let mut time = 0.0;
        let mut last_beats = 0.0;
        let mut last_bpm: Option<f32> = None;
        for (now_beats, bpm) in ranges {
            if let Some(bpm) = last_bpm {
                time += (now_beats - last_beats) * (60. / bpm);
            }
            last_beats = now_beats;
            last_bpm = Some(bpm);
            elements.push((now_beats, time, bpm));
        }
        BpmList {
            elements,
            cursor: 0,
        }
    }

    pub fn time_beats(&mut self, beats: f32) -> f32 {
        while let Some(kf) = self.elements.get(self.cursor + 1) {
            if kf.0 > beats {
                break;
            }
            self.cursor += 1;
        }
        while self.cursor != 0 && self.elements[self.cursor].0 > beats {
            self.cursor -= 1;
        }
        let (start_beats, time, bpm) = &self.elements[self.cursor];
        time + (beats - start_beats) * (60. / bpm)
    }

    pub fn time(&mut self, triple: &Triple) -> f32 {
        self.time_beats(triple.beats())
    }
}
