pub struct TimeManager {
    adjust_time: bool,
    start_time: f64,
    pause_time: Option<f64>,
    wait: f64,
    velocity: f64,

    get_time_fn: Box<dyn Fn() -> f64>,

    pub time: f64,
}

impl TimeManager {
    pub fn new(adjust_time: bool, get_time_fn: Box<dyn Fn() -> f64>) -> Self {
        let t = get_time_fn();
        Self {
            adjust_time,
            start_time: t,
            pause_time: None,
            wait: f64::NEG_INFINITY,
            velocity: 0.,

            get_time_fn,

            time: 0.,
        }
    }

    pub fn real_time(&self) -> f64 {
        (self.get_time_fn)()
    }

    pub fn wait(&mut self) {
        self.wait = self.real_time() + 0.1;
    }

    pub fn update(&mut self, music_time: f64) {
        let t = self.real_time();
        self.time = self.pause_time.unwrap_or(t) - self.start_time;
        if self.adjust_time && t > self.wait && self.pause_time.is_none() {
            self.start_time -= (music_time - self.time) * 3e-3;
        }
    }

    pub fn paused(&self) -> bool {
        self.pause_time.is_some()
    }

    pub fn pause(&mut self) {
        self.pause_time = Some(self.real_time());
        self.velocity = 0.;
    }

    pub fn resume(&mut self) {
        self.start_time += self.real_time() - self.pause_time.take().unwrap();
        self.velocity = 0.;
        self.wait();
    }

    pub fn seek_to(&mut self, pos: f64) {
        self.start_time = self.real_time() - pos;
        self.time = pos;
        self.velocity = 0.;
        self.wait();
    }
}
