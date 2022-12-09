use crate::{
    core::{
        BadNote, Chart, NoteKind, Point, Resource, Vector, JUDGE_LINE_GOOD_COLOR,
        JUDGE_LINE_PERFECT_COLOR, NOTE_WIDTH_RATIO,
    },
    ext::NotNanExt,
};
use macroquad::prelude::{
    utils::{register_input_subscriber, repeat_all_miniquad_input},
    *,
};
use miniquad::{EventHandler, MouseButton};
use std::collections::{HashMap, VecDeque};

const X_DIFF_MAX: f32 = 1.9 * NOTE_WIDTH_RATIO;

const FLICK_SPEED_THRESHOLD: f32 = 2.7;
const LIMIT_PERFECT: f32 = 0.08;
const LIMIT_GOOD: f32 = 0.18;
const LIMIT_BAD: f32 = 0.22;

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
        // println!("PUSH {} {}", time, position);
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
        warn!(
            "{norm} {}",
            (self.last_dir.dot(&spd.unscale(norm)) - 1.).abs()
        );
        if self.wait && (norm <= 0.5 || (self.last_dir.dot(&spd.unscale(norm)) - 1.).abs() > 0.4) {
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
    Hold(bool, f32, bool), // perfect, at, pre-judge
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
    notes: Vec<(Vec<usize>, usize)>,
    trackers: HashMap<u64, VelocityTracker>,
    subscriber_id: usize,
    last_time: f32,

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
                let mut idx: Vec<usize> = (0..line.notes.len())
                    .filter(|it| !line.notes[*it].fake)
                    .collect();
                idx.sort_by_key(|id| line.notes[*id].time.not_nan());
                (idx, 0)
            })
            .collect();
        Self {
            notes,
            trackers: HashMap::new(),
            subscriber_id: register_input_subscriber(),
            last_time: 0.,

            combo: 0,
            max_combo: 0,
            counts: [0; 4],
            num_of_notes: chart
                .lines
                .iter()
                .map(|it| it.notes.iter().filter(|it| !it.fake).count() as u32)
                .sum(),
        }
    }

    pub fn commit(&mut self, what: Judgement) {
        use Judgement::*;
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

    pub fn score(&self) -> u32 {
        if self.counts[0] == self.num_of_notes {
            1000000
        } else {
            let score = (9.0 * (self.counts[0] as f64 + self.counts[1] as f64 * 0.65)
                + self.max_combo as f64)
                * (100000.0 / self.num_of_notes as f64);
            score.round() as u32
        }
    }

    pub fn update(&mut self, res: &mut Resource, chart: &mut Chart, bad_notes: &mut Vec<BadNote>) {
        if res.config.autoplay {
            self.auto_play_update(res, chart);
            return;
        }
        let t = res.time;
        let mut touches = touches_local();
        if !touches.is_empty() {
            warn!("{:?}", touches);
        }
        {
            // TODO not complete
            let btn = MouseButton::Left;
            if is_mouse_button_pressed(btn) {
                touches.push(Touch {
                    id: u64::MAX,
                    phase: TouchPhase::Started,
                    position: mouse_position_local(),
                });
            } else if is_mouse_button_down(btn) {
                touches.push(Touch {
                    id: u64::MAX,
                    phase: TouchPhase::Moved,
                    position: mouse_position_local(),
                });
            } else if is_mouse_button_released(btn) {
                touches.push(Touch {
                    id: u64::MAX,
                    phase: TouchPhase::Ended,
                    position: mouse_position_local(),
                });
            }
        }
        // TODO optimize
        let mut touches: HashMap<u64, Touch> = touches.into_iter().map(|it| (it.id, it)).collect();
        let events = {
            let mut handler = Handler(Vec::new());
            repeat_all_miniquad_input(&mut handler, self.subscriber_id);
            handler.0
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
        let pos: Vec<Vec<Option<Point>>> = chart
            .lines
            .iter()
            .map(|line| {
                let inv = line.object.now(res).try_inverse().unwrap();
                touches
                    .iter()
                    .map(|touch| {
                        let p = touch.position;
                        let p = inv.transform_point(&Point::new(p.x, -p.y));
                        if !p.x.is_normal() || !p.y.is_normal() {
                            None
                        } else {
                            Some(p)
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
                    Moved | Stationary => self
                        .trackers
                        .get_mut(&touch.id)
                        .map_or(false, |it| it.has_flick()),
                    _ => false,
                } {
                    |kind: &NoteKind| matches!(kind, NoteKind::Flick)
                } else {
                    continue; // to next touch
                }
            };
            let mut closest = (None, X_DIFF_MAX, LIMIT_BAD);
            for (line_id, ((line, pos), (idx, st))) in chart
                .lines
                .iter_mut()
                .zip(pos.iter())
                .zip(self.notes.iter_mut())
                .enumerate()
            {
                let Some(pos) = pos[id] else { continue; };
                for id in &idx[*st..] {
                    let note = &mut line.notes[*id];
                    if !matches!(note.judge, JudgeStatus::NotJudged) {
                        continue;
                    }
                    if !filter(&note.kind) {
                        continue;
                    }
                    if note.time - t >= closest.2 {
                        break;
                    }
                    if t - note.time >= closest.2 {
                        continue;
                    }
                    let x = &mut note.object.translation.0;
                    x.set_time(t);
                    let dist = (x.now() - pos.x).abs();
                    let dt = (note.time - t).abs();
                    let bad = LIMIT_BAD - LIMIT_PERFECT * (dist - 0.9).max(0.);
                    if dt > bad {
                        continue;
                    }
                    if dist < closest.1 {
                        closest.0 = Some((line_id, *id));
                        closest.1 = dist;
                        closest.2 = dt + 0.01;
                    }
                }
            }
            if let (Some((line_id, id)), _, dt) = closest {
                let line = &mut chart.lines[line_id];
                if matches!(touch.phase, TouchPhase::Started) {
                    // click & hold
                    if dt <= LIMIT_GOOD {
                        match line.notes[id].kind {
                            NoteKind::Click => {
                                line.notes[id].judge = JudgeStatus::Judged;
                                judgements.push((
                                    if dt <= LIMIT_PERFECT {
                                        Judgement::Perfect
                                    } else {
                                        Judgement::Good
                                    },
                                    line_id,
                                    id,
                                ));
                            }
                            NoteKind::Hold { .. } => {
                                res.play_sfx(&res.sfx_click.clone());
                                line.notes[id].judge =
                                    JudgeStatus::Hold(dt <= LIMIT_PERFECT, t, false);
                            }
                            _ => unreachable!(),
                        };
                    } else {
                        line.notes[id].judge = JudgeStatus::Judged;
                        judgements.push((Judgement::Bad, line_id, id));
                    }
                } else {
                    // flick
                    line.notes[id].judge = JudgeStatus::PreJudge;
                }
            }
        }
        for (line_id, ((line, pos), (idx, st))) in chart
            .lines
            .iter_mut()
            .zip(pos.iter())
            .zip(self.notes.iter())
            .enumerate()
        {
            line.object.set_time(t);
            for id in &idx[*st..] {
                let note = &mut line.notes[*id];
                if let NoteKind::Hold { end_time, .. } = &note.kind {
                    if let JudgeStatus::Hold(.., ref mut pre_judge) = note.judge {
                        if t + LIMIT_BAD >= *end_time {
                            *pre_judge = true;
                            continue;
                        }
                        let x = &mut note.object.translation.0;
                        x.set_time(t);
                        let x = x.now();
                        if !pos
                            .iter()
                            .any(|it| it.map_or(false, |it| (it.x - x).abs() <= X_DIFF_MAX))
                        {
                            note.judge = JudgeStatus::Judged;
                            judgements.push((Judgement::Miss, line_id, *id));
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
                    judgements.push((Judgement::Miss, line_id, *id));
                    continue;
                }
                if note.time > t + LIMIT_BAD {
                    break;
                }
                if !matches!(note.kind, NoteKind::Drag) {
                    continue;
                }

                let dt = (t - note.time).abs();
                let x = &mut note.object.translation.0;
                x.set_time(t);
                let x = x.now();
                if pos.iter().any(|it| {
                    it.map_or(false, |it| {
                        let dx = (it.x - x).abs();
                        dx <= X_DIFF_MAX && dt <= (LIMIT_BAD - LIMIT_PERFECT * (dx - 0.9).max(0.))
                    })
                }) {
                    note.judge = JudgeStatus::PreJudge;
                }
            }
        }
        // process pre-judge
        for (line_id, (line, (idx, st))) in
            chart.lines.iter_mut().zip(self.notes.iter()).enumerate()
        {
            line.object.set_time(t);
            for id in &idx[*st..] {
                let note = &mut line.notes[*id];
                if matches!(note.judge, JudgeStatus::Hold(.., true)) {
                    if let NoteKind::Hold { end_time, .. } = &note.kind {
                        if *end_time <= t {
                            note.judge = JudgeStatus::Judged;
                            judgements.push((Judgement::Perfect, line_id, *id));
                            continue;
                        }
                    }
                }
                if t < note.time {
                    break;
                }
                if matches!(note.judge, JudgeStatus::PreJudge) {
                    note.judge = JudgeStatus::Judged;
                    judgements.push((Judgement::Perfect, line_id, *id));
                }
            }
        }
        for (judgement, line_id, id) in judgements.into_iter() {
            self.commit(judgement);
            let line = &chart.lines[line_id];
            let note = &line.notes[id];
            if matches!(note.kind, NoteKind::Hold { .. }) {
                continue;
            }
            if match judgement {
                Judgement::Perfect => {
                    res.with_model(line.object.now(res) * note.object.now(res), |res| {
                        res.emit_at_origin(JUDGE_LINE_PERFECT_COLOR)
                    });
                    true
                }
                Judgement::Good => {
                    res.with_model(line.object.now(res) * note.object.now(res), |res| {
                        res.emit_at_origin(JUDGE_LINE_GOOD_COLOR)
                    });
                    true
                }
                Judgement::Bad => {
                    if !matches!(note.kind, NoteKind::Hold { .. }) {
                        bad_notes.push(BadNote {
                            time: t,
                            kind: note.kind.clone(),
                            matrix: line.object.now(res)
                                * note.now_transform(
                                    res,
                                    (note.height - line.height.now()) / res.config.aspect_ratio
                                        * note.speed,
                                ),
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
            while idx.get(*st).map_or(false, |id| {
                matches!(line.notes[*id].judge, JudgeStatus::Judged)
            }) {
                *st += 1;
            }
        }
        self.last_time = t;
    }

    fn auto_play_update(&mut self, res: &mut Resource, chart: &mut Chart) {
        let t = res.time;
        let mut judgements = Vec::new();
        for (line_id, (line, (idx, st))) in chart
            .lines
            .iter_mut()
            .zip(self.notes.iter_mut())
            .enumerate()
        {
            for id in &idx[*st..] {
                let note = &mut line.notes[*id];
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
                    JudgeStatus::Hold(true, t, false)
                } else {
                    judgements.push((line_id, *id));
                    JudgeStatus::Judged
                };
            }
            while idx.get(*st).map_or(false, |id| {
                matches!(line.notes[*id].judge, JudgeStatus::Judged)
            }) {
                *st += 1;
            }
        }
        for (line_id, id) in judgements.into_iter() {
            self.commit(Judgement::Perfect);
            let line = &chart.lines[line_id];
            let note = &line.notes[id];
            res.with_model(line.object.now(res) * note.object.now(res), |res| {
                res.emit_at_origin(JUDGE_LINE_PERFECT_COLOR)
            });
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
}

struct Handler(Vec<(u64, miniquad::TouchPhase, (f32, f32))>);

impl EventHandler for Handler {
    fn update(&mut self, _: &mut miniquad::Context) {}
    fn draw(&mut self, _: &mut miniquad::Context) {}
    fn touch_event(
        &mut self,
        _: &mut miniquad::Context,
        phase: miniquad::TouchPhase,
        id: u64,
        x: f32,
        y: f32,
    ) {
        self.0.push((id, phase, (x, y)));
    }
}
