use crate::{
    core::{BadNote, Chart, NoteKind, Point, Resource, Vector, JUDGE_LINE_GOOD_COLOR, JUDGE_LINE_PERFECT_COLOR, NOTE_WIDTH_RATIO},
    ext::NotNanExt,
};
use macroquad::prelude::{
    utils::{register_input_subscriber, repeat_all_miniquad_input},
    *,
};
use miniquad::{EventHandler, MouseButton};
use std::{
    collections::{HashMap, VecDeque},
    num::FpCategory,
};

const X_DIFF_MAX: f32 = 1.9 * NOTE_WIDTH_RATIO;

pub const FLICK_SPEED_THRESHOLD: f32 = 2.2;
pub const LIMIT_PERFECT: f32 = 0.08;
pub const LIMIT_GOOD: f32 = 0.18;
pub const LIMIT_BAD: f32 = 0.22;

pub struct VelocityTracker {
    movements: VecDeque<(f32, Point)>,
    pub start_time: f32,
    sum_x: f32,
    sum_x2: f32,
    sum_x3: f32,
    sum_x4: f32,
    sum_y: Point,
    sum_x_y: Point,
    sum_x2_y: Point,
    last_dir: Vector,
    wait: bool,
}

impl VelocityTracker {
    pub const RECORD_MAX: usize = 20;

    pub fn new(time: f32, point: Point) -> Self {
        let mut res = Self {
            movements: VecDeque::with_capacity(Self::RECORD_MAX),
            start_time: time,
            // TODO simplify
            sum_x: 0.0,
            sum_x2: 0.0,
            sum_x3: 0.0,
            sum_x4: 0.0,
            sum_y: Point::default(),
            sum_x_y: Point::default(),
            sum_x2_y: Point::default(),
            last_dir: Vector::default(),
            wait: false,
        };
        res.push(time, point);
        res
    }

    fn update<const C: i32>(&mut self, (time, position): (f32, Point)) {
        let position = position.coords;
        let c = C as f32;
        self.sum_y += position * c;
        let mut cur = time * c;
        self.sum_x += cur;
        self.sum_x_y += position * cur;
        cur *= time;
        self.sum_x2 += cur;
        self.sum_x2_y += position * cur;
        cur *= time;
        self.sum_x3 += cur;
        self.sum_x4 += cur * time;
    }

    pub fn push(&mut self, time: f32, position: Point) {
        let time = time - self.start_time;
        if self.movements.len() == Self::RECORD_MAX {
            let pair = self.movements.pop_front().unwrap();
            self.update::<-1>(pair);
        }
        let pair = (time, position);
        self.movements.push_back(pair);
        self.update::<1>(pair);
    }

    pub fn speed(&self) -> Vector {
        if self.movements.is_empty() {
            return Vector::default();
        }
        let n = self.movements.len() as f32;
        let s_xx = self.sum_x2 - self.sum_x * self.sum_x / n;
        let s_xy = self.sum_x_y - self.sum_y * (self.sum_x / n);
        let s_xx2 = self.sum_x3 - self.sum_x * self.sum_x2 / n;
        let s_x2y = self.sum_x2_y - self.sum_y * (self.sum_x2 / n);
        let s_x2x2 = self.sum_x4 - self.sum_x2 * self.sum_x2 / n;
        let denom = s_xx * s_x2x2 - s_xx2 * s_xx2;
        if denom == 0.0 {
            return Vector::default();
        }
        let a = (s_x2y * s_xx - s_xy * s_xx2) / denom;
        let b = (s_xy * s_x2x2 - s_x2y * s_xx2) / denom;
        // let c = (self.sum_y - b * self.sum_x - a * self.sum_x2) / n;
        let x = self.movements.back().unwrap().0;
        a * (x * 2.0) + b
    }

    pub fn has_flick(&mut self) -> bool {
        let spd = self.speed();
        let norm = spd.norm();
        if self.wait && (norm <= 1.2 || (self.last_dir.dot(&spd.unscale(norm)) - 1.).abs() > 0.4) {
            self.wait = false;
        }
        if self.wait {
            return false;
        }
        if norm >= FLICK_SPEED_THRESHOLD {
            self.last_dir = spd.unscale(norm);
            self.wait = true;
            true
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub enum JudgeStatus {
    NotJudged,
    PreJudge,
    Judged,
    Hold(bool, f32, f32, bool), // perfect, at, diff, pre-judge
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum Judgement {
    Perfect,
    Good,
    Bad,
    Miss,
}

pub struct Judge {
    // notes of each line in order
    // LinkedList::drain_filter is unstable...
    notes: Vec<(Vec<u32>, usize)>,
    trackers: HashMap<u64, VelocityTracker>,
    subscriber_id: usize,
    last_time: f32,
    key_down_count: u32,
    diffs: Vec<f32>,

    pub combo: u32,
    pub max_combo: u32,
    pub counts: [u32; 4],
    pub num_of_notes: u32,
}

impl Judge {
    pub fn new(chart: &Chart) -> Self {
        let notes = chart
            .lines
            .iter()
            .map(|line| {
                let mut idx: Vec<u32> = (0..(line.notes.len() as u32)).filter(|it| !line.notes[*it as usize].fake).collect();
                idx.sort_by_key(|id| line.notes[*id as usize].time.not_nan());
                (idx, 0)
            })
            .collect();
        Self {
            notes,
            trackers: HashMap::new(),
            subscriber_id: register_input_subscriber(),
            last_time: 0.,
            key_down_count: 0,
            diffs: Vec::new(),

            combo: 0,
            max_combo: 0,
            counts: [0; 4],
            num_of_notes: chart.lines.iter().map(|it| it.notes.iter().filter(|it| !it.fake).count() as u32).sum(),
        }
    }

    pub fn reset(&mut self) {
        self.notes.iter_mut().for_each(|it| it.1 = 0);
        self.trackers.clear();
        self.combo = 0;
        self.max_combo = 0;
        self.counts = [0; 4];
        self.diffs.clear();
    }

    pub fn commit(&mut self, what: Judgement, diff: Option<f32>) {
        use Judgement::*;
        if let Some(diff) = diff {
            self.diffs.push(diff);
        }
        self.counts[what as usize] += 1;
        match what {
            Perfect | Good => {
                self.combo += 1;
                if self.combo > self.max_combo {
                    self.max_combo = self.combo;
                }
            }
            _ => {
                self.combo = 0;
            }
        }
    }

    pub fn accuracy(&self) -> f64 {
        (self.counts[0] as f64 + self.counts[1] as f64 * 0.65) / self.num_of_notes as f64
    }

    pub fn score(&self) -> u32 {
        if self.counts[0] == self.num_of_notes {
            1000000
        } else {
            let score = (9.0 * self.accuracy() + self.max_combo as f64 / self.num_of_notes as f64) * 100000.0;
            score.round() as u32
        }
    }

    pub fn touches_raw() -> Vec<Touch> {
        let mut touches = touches();
        // TODO not complete
        let btn = MouseButton::Left;
        if is_mouse_button_pressed(btn) {
            let p = mouse_position();
            touches.push(Touch {
                id: u64::MAX,
                phase: TouchPhase::Started,
                position: vec2(p.0, p.1),
            });
        } else if is_mouse_button_down(btn) {
            let p = mouse_position();
            touches.push(Touch {
                id: u64::MAX,
                phase: TouchPhase::Moved,
                position: vec2(p.0, p.1),
            });
        } else if is_mouse_button_released(btn) {
            let p = mouse_position();
            touches.push(Touch {
                id: u64::MAX,
                phase: TouchPhase::Ended,
                position: vec2(p.0, p.1),
            });
        }
        touches
    }

    pub fn get_touches(res: &Resource) -> Vec<Touch> {
        let touches = Self::touches_raw();
        let vp = unsafe { get_internal_gl() }.quad_gl.get_viewport();
        touches
            .into_iter()
            .map(|mut touch| {
                let p = touch.position;
                touch.position = vec2((p.x - vp.0 as f32) / vp.2 as f32 * 2. - 1., ((p.y - vp.1 as f32) / vp.3 as f32 * 2. - 1.) / res.aspect_ratio);
                touch
            })
            .collect()
    }

    pub fn update(&mut self, res: &mut Resource, chart: &mut Chart, bad_notes: &mut Vec<BadNote>) {
        if res.config.autoplay {
            self.auto_play_update(res, chart);
            return;
        }
        let t = res.time;
        let touches = Self::get_touches(res);
        // TODO optimize
        let mut touches: HashMap<u64, Touch> = touches.into_iter().map(|it| (it.id, it)).collect();
        let (events, keys_down) = {
            let mut handler = Handler(Vec::new(), &mut self.key_down_count, 0);
            repeat_all_miniquad_input(&mut handler, self.subscriber_id);
            (handler.0, handler.2)
        };
        {
            fn to_local((x, y): (f32, f32)) -> Point {
                Point::new(x / screen_width() * 2. - 1., y / screen_height() * 2. - 1.)
            }
            let delta = (t - self.last_time) as f64 / (events.len() + 1) as f64;
            let mut t = self.last_time as f64;
            for (id, phase, p) in events.into_iter() {
                t += delta;
                let t = t as f32;
                let p = to_local(p);
                match phase {
                    miniquad::TouchPhase::Started => {
                        self.trackers.insert(id, VelocityTracker::new(t, p));
                        touches
                            .entry(id)
                            .or_insert_with(|| Touch {
                                id,
                                phase: TouchPhase::Started,
                                position: vec2(p.x, p.y),
                            })
                            .phase = TouchPhase::Started;
                    }
                    miniquad::TouchPhase::Moved => {
                        if let Some(tracker) = self.trackers.get_mut(&id) {
                            tracker.push(t, p);
                        }
                    }
                    miniquad::TouchPhase::Ended | miniquad::TouchPhase::Cancelled => {
                        self.trackers.remove(&id);
                    }
                }
            }
        }
        let touches: Vec<Touch> = touches.into_values().collect();
        // pos[line][touch]
        let mut index = 0;
        let pos: Vec<Vec<Option<Point>>> = chart
            .lines
            .iter_mut()
            .map(|line| {
                index += 1;
                line.object.set_time(t);
                let inv = line.object.now(res).try_inverse().unwrap();
                touches
                    .iter()
                    .map(|touch| {
                        let p = touch.position;
                        let p = inv.transform_point(&Point::new(p.x, -p.y));
                        fn ok(f: f32) -> bool {
                            matches!(f.classify(), FpCategory::Zero | FpCategory::Subnormal | FpCategory::Normal)
                        }
                        if ok(p.x) && ok(p.y) {
                            Some(p)
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .collect();
        let mut judgements = Vec::new();
        // clicks & flicks
        for (id, touch) in touches.iter().enumerate() {
            // TODO optimize?
            let filter = if matches!(touch.phase, TouchPhase::Started) {
                |kind: &NoteKind| matches!(kind, NoteKind::Click | NoteKind::Hold { .. })
            } else {
                // check for flicks
                use TouchPhase::*;
                if match touch.phase {
                    Moved | Stationary => self.trackers.get_mut(&touch.id).map_or(false, |it| it.has_flick()),
                    _ => false,
                } {
                    |kind: &NoteKind| matches!(kind, NoteKind::Flick)
                } else {
                    continue; // to next touch
                }
            };
            let mut closest = (None, X_DIFF_MAX, LIMIT_BAD);
            for (line_id, ((line, pos), (idx, st))) in chart.lines.iter_mut().zip(pos.iter()).zip(self.notes.iter_mut()).enumerate() {
                let Some(pos) = pos[id] else { continue; };
                for id in &idx[*st..] {
                    let note = &mut line.notes[*id as usize];
                    if !matches!(note.judge, JudgeStatus::NotJudged) {
                        continue;
                    }
                    if !filter(&note.kind) {
                        continue;
                    }
                    if note.time - t >= closest.2 {
                        break;
                    }
                    let x = &mut note.object.translation.0;
                    x.set_time(t);
                    let dist = (x.now() - pos.x).abs();
                    let dt = (note.time - t).abs();
                    let bad = LIMIT_BAD - LIMIT_PERFECT * (dist - 0.9).max(0.);
                    if dt > bad {
                        continue;
                    }
                    if dist < NOTE_WIDTH_RATIO || dist < closest.1 {
                        closest.0 = Some((line_id, *id));
                        closest.1 = dist;
                        closest.2 = note.time - t + 0.01;
                        if dist < NOTE_WIDTH_RATIO {
                            break;
                        }
                    }
                }
            }
            if let (Some((line_id, id)), _, dt) = closest {
                let line = &mut chart.lines[line_id];
                if matches!(touch.phase, TouchPhase::Started) {
                    // click & hold
                    let note = &mut line.notes[id as usize];
                    let dt = dt.abs();
                    if dt <= LIMIT_GOOD {
                        match note.kind {
                            NoteKind::Click => {
                                note.judge = JudgeStatus::Judged;
                                judgements.push((if dt <= LIMIT_PERFECT { Judgement::Perfect } else { Judgement::Good }, line_id, id, None));
                            }
                            NoteKind::Hold { .. } => {
                                res.play_sfx(&res.sfx_click.clone());
                                note.judge = JudgeStatus::Hold(dt <= LIMIT_PERFECT, t, t - note.time, false);
                            }
                            _ => unreachable!(),
                        };
                    } else {
                        line.notes[id as usize].judge = JudgeStatus::Judged;
                        judgements.push((Judgement::Bad, line_id, id, None));
                    }
                } else {
                    // flick
                    line.notes[id as usize].judge = JudgeStatus::PreJudge;
                }
            }
        }
        for _ in 0..keys_down {
            // find the earliest not judged click / hold note
            if let Some((line_id, id)) = chart
                .lines
                .iter()
                .zip(self.notes.iter())
                .enumerate()
                .filter_map(|(line_id, (line, (idx, st)))| {
                    idx[*st as usize..]
                        .iter()
                        .cloned()
                        .find(|id| {
                            let note = &line.notes[*id as usize];
                            matches!(note.judge, JudgeStatus::NotJudged) && matches!(note.kind, NoteKind::Click | NoteKind::Hold { .. })
                        })
                        .map(|id| (line_id, id))
                })
                .min_by_key(|(line_id, id)| chart.lines[*line_id].notes[*id as usize].time.not_nan())
            {
                let note = &mut chart.lines[line_id].notes[id as usize];
                let dt = (t - note.time).abs();
                if dt <= LIMIT_BAD {
                    match note.kind {
                        NoteKind::Click => {
                            note.judge = JudgeStatus::Judged;
                            judgements.push((if dt <= LIMIT_PERFECT { Judgement::Perfect } else { Judgement::Good }, line_id, id, None));
                        }
                        NoteKind::Hold { .. } => {
                            res.play_sfx(&res.sfx_click.clone());
                            note.judge = JudgeStatus::Hold(dt <= LIMIT_PERFECT, t, t - note.time, false);
                        }
                        _ => unreachable!(),
                    };
                }
            }
        }
        for (line_id, ((line, pos), (idx, st))) in chart.lines.iter_mut().zip(pos.iter()).zip(self.notes.iter()).enumerate() {
            line.object.set_time(t);
            for id in &idx[*st..] {
                let note = &mut line.notes[*id as usize];
                if let NoteKind::Hold { end_time, .. } = &note.kind {
                    if let JudgeStatus::Hold(.., ref mut pre_judge) = note.judge {
                        if t + LIMIT_BAD >= *end_time {
                            *pre_judge = true;
                            continue;
                        }
                        let x = &mut note.object.translation.0;
                        x.set_time(t);
                        let x = x.now();
                        if self.key_down_count == 0 && !pos.iter().any(|it| it.map_or(false, |it| (it.x - x).abs() <= X_DIFF_MAX)) {
                            note.judge = JudgeStatus::Judged;
                            judgements.push((Judgement::Miss, line_id, *id, None));
                            continue;
                        }
                    }
                }
                if !matches!(note.judge, JudgeStatus::NotJudged) {
                    continue;
                }
                // process miss
                if note.time < t - LIMIT_BAD {
                    note.judge = JudgeStatus::Judged;
                    judgements.push((Judgement::Miss, line_id, *id, None));
                    continue;
                }
                if note.time > t + LIMIT_BAD {
                    break;
                }
                if !matches!(note.kind, NoteKind::Drag) && (self.key_down_count == 0 || !matches!(note.kind, NoteKind::Flick)) {
                    continue;
                }
                let dt = (t - note.time).abs();
                let x = &mut note.object.translation.0;
                x.set_time(t);
                let x = x.now();
                if self.key_down_count != 0
                    || pos.iter().any(|it| {
                        it.map_or(false, |it| {
                            let dx = (it.x - x).abs();
                            dx <= X_DIFF_MAX && dt <= (LIMIT_BAD - LIMIT_PERFECT * (dx - 0.9).max(0.))
                        })
                    })
                {
                    note.judge = JudgeStatus::PreJudge;
                }
            }
        }
        // process pre-judge
        for (line_id, (line, (idx, st))) in chart.lines.iter_mut().zip(self.notes.iter()).enumerate() {
            line.object.set_time(t);
            for id in &idx[*st..] {
                let note = &mut line.notes[*id as usize];
                if let JudgeStatus::Hold(perfect, .., diff, true) = note.judge {
                    if let NoteKind::Hold { end_time, .. } = &note.kind {
                        if *end_time <= t {
                            note.judge = JudgeStatus::Judged;
                            judgements.push((if perfect { Judgement::Perfect } else { Judgement::Good }, line_id, *id, Some(diff)));
                            continue;
                        }
                    }
                }
                if t < note.time {
                    break;
                }
                if matches!(note.judge, JudgeStatus::PreJudge) {
                    let diff = if let JudgeStatus::Hold(.., diff, _) = note.judge {
                        Some(diff)
                    } else {
                        None
                    };
                    note.judge = JudgeStatus::Judged;
                    judgements.push((Judgement::Perfect, line_id, *id, diff));
                }
            }
        }
        for (judgement, line_id, id, diff) in judgements.into_iter() {
            let line = &chart.lines[line_id];
            let note = &line.notes[id as usize];
            self.commit(
                judgement,
                if matches!(judgement, Judgement::Good | Judgement::Bad) {
                    Some(diff.unwrap_or(t - note.time))
                } else {
                    None
                },
            );
            if matches!(note.kind, NoteKind::Hold { .. }) {
                continue;
            }
            if match judgement {
                Judgement::Perfect => {
                    res.with_model(line.object.now(res) * note.object.now(res), |res| res.emit_at_origin(JUDGE_LINE_PERFECT_COLOR));
                    true
                }
                Judgement::Good => {
                    res.with_model(line.object.now(res) * note.object.now(res), |res| res.emit_at_origin(JUDGE_LINE_GOOD_COLOR));
                    true
                }
                Judgement::Bad => {
                    if !matches!(note.kind, NoteKind::Hold { .. }) {
                        bad_notes.push(BadNote {
                            time: t,
                            kind: note.kind.clone(),
                            matrix: line.object.now(res) * note.now_transform(res, (note.height - line.height.now()) / res.aspect_ratio * note.speed),
                            speed: Vector::default(),
                        });
                    }
                    false
                }
                _ => false,
            } {
                if let Some(sfx) = match note.kind {
                    NoteKind::Click => Some(&res.sfx_click),
                    NoteKind::Drag => Some(&res.sfx_drag),
                    NoteKind::Flick => Some(&res.sfx_flick),
                    _ => None,
                } {
                    res.play_sfx(&sfx.clone());
                }
            }
        }
        for (line, (idx, st)) in chart.lines.iter().zip(self.notes.iter_mut()) {
            while idx
                .get(*st)
                .map_or(false, |id| matches!(line.notes[*id as usize].judge, JudgeStatus::Judged))
            {
                *st += 1;
            }
        }
        self.last_time = t;
    }

    fn auto_play_update(&mut self, res: &mut Resource, chart: &mut Chart) {
        let t = res.time;
        let mut judgements = Vec::new();
        for (line_id, (line, (idx, st))) in chart.lines.iter_mut().zip(self.notes.iter_mut()).enumerate() {
            for id in &idx[*st..] {
                let note = &mut line.notes[*id as usize];
                if let JudgeStatus::Hold(..) = note.judge {
                    if let NoteKind::Hold { end_time, .. } = note.kind {
                        if t >= end_time {
                            note.judge = JudgeStatus::Judged;
                            judgements.push((line_id, *id));
                            continue;
                        }
                    }
                }
                if !matches!(note.judge, JudgeStatus::NotJudged) {
                    continue;
                }
                if note.time > t {
                    break;
                }
                note.judge = if matches!(note.kind, NoteKind::Hold { .. }) {
                    res.play_sfx(&res.sfx_click.clone());
                    JudgeStatus::Hold(true, t, t - note.time, false)
                } else {
                    judgements.push((line_id, *id));
                    JudgeStatus::Judged
                };
            }
            while idx
                .get(*st)
                .map_or(false, |id| matches!(line.notes[*id as usize].judge, JudgeStatus::Judged))
            {
                *st += 1;
            }
        }
        for (line_id, id) in judgements.into_iter() {
            self.commit(Judgement::Perfect, None);
            let line = &chart.lines[line_id];
            let note = &line.notes[id as usize];
            res.with_model(line.object.now(res) * note.object.now(res), |res| res.emit_at_origin(JUDGE_LINE_PERFECT_COLOR));
            if let Some(sfx) = match note.kind {
                NoteKind::Click => Some(&res.sfx_click),
                NoteKind::Drag => Some(&res.sfx_drag),
                NoteKind::Flick => Some(&res.sfx_flick),
                _ => None,
            } {
                res.play_sfx(&sfx.clone());
            }
        }
    }

    pub fn result(&self) -> PlayResult {
        let early = self.diffs.iter().filter(|it| **it < 0.).count() as u32;
        PlayResult {
            score: self.score(),
            accuracy: self.accuracy(),
            max_combo: self.max_combo,
            num_of_notes: self.num_of_notes,
            counts: self.counts,
            early,
            late: self.diffs.len() as u32 - early,
        }
    }
}

struct Handler<'a>(Vec<(u64, miniquad::TouchPhase, (f32, f32))>, &'a mut u32, u32);

impl<'a> EventHandler for Handler<'a> {
    fn update(&mut self, _: &mut miniquad::Context) {}
    fn draw(&mut self, _: &mut miniquad::Context) {}
    fn touch_event(&mut self, _: &mut miniquad::Context, phase: miniquad::TouchPhase, id: u64, x: f32, y: f32) {
        self.0.push((id, phase, (x, y)));
    }

    fn key_down_event(&mut self, _ctx: &mut miniquad::Context, _keycode: KeyCode, _keymods: miniquad::KeyMods, repeat: bool) {
        if !repeat {
            *self.1 += 1;
            self.2 += 1;
        }
    }

    fn key_up_event(&mut self, _ctx: &mut miniquad::Context, _keycode: KeyCode, _keymods: miniquad::KeyMods) {
        *self.1 -= 1;
    }
}

#[derive(Default)]
pub struct PlayResult {
    pub score: u32,
    pub accuracy: f64,
    pub max_combo: u32,
    pub num_of_notes: u32,
    pub counts: [u32; 4],
    pub early: u32,
    pub late: u32,
}
