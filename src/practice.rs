use std::time::{Duration, Instant};

use crate::song::Chart;

pub const MISTAKE_FLASH: Duration = Duration::from_millis(300);
// Hit windows are in real seconds, so they feel the same at every speed.
const PERFECT_WINDOW: f32 = 0.07;
const LEAD_IN_SECONDS: f32 = 2.0;
// Lowest note on the letter rows (F3, the Q key) and the top of the keyboard.
const LOWEST_LETTER_KEY: u8 = 53;
const HIGHEST_KEY: u8 = 96;
const NOTE_GRID_BEATS: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];

/// How many melody notes the player types. The rest play by themselves.
#[derive(Clone, Copy, PartialEq)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Self; 3] = [Self::Easy, Self::Normal, Self::Hard];

    pub fn name(self) -> &'static str {
        match self {
            Self::Easy => "Easy",
            Self::Normal => "Normal",
            Self::Hard => "Hard",
        }
    }

    // How far off a hit can be and still count.
    fn good_window(self) -> f32 {
        match self {
            Self::Easy => 0.25,
            Self::Normal | Self::Hard => 0.16,
        }
    }

    // Shortest real-time gap between notes the player has to type.
    fn min_seconds(self) -> f32 {
        match self {
            Self::Easy => 0.6,
            Self::Normal => 0.3,
            Self::Hard => 0.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Judgement {
    Perfect,
    Good,
    Miss,
}

/// A melody note for the player to hit. Times are in song seconds at 100% speed.
pub struct Target {
    pub time: f32,
    pub length: f32,
    pub key: u8,
    pub judgement: Option<Judgement>,
    /// Played automatically because the difficulty leaves it out.
    pub auto: bool,
}

impl Target {
    fn is_open(&self) -> bool {
        !self.auto && self.judgement.is_none()
    }
}

struct Backing {
    time: f32,
    length: f32,
    key: u8,
}

pub struct Scheduled {
    pub delay: f32,
    pub key: u8,
    pub length: f32,
}

pub enum Press {
    Hit(Scheduled),
    Miss,
}

/// A song played as a rhythm game: the accompaniment plays itself on a steady clock and the
/// player hits the melody notes as they arrive.
pub struct Practice {
    pub title: String,
    pub perfect: u32,
    pub good: u32,
    pub missed: u32,
    pub streak: u32,
    targets: Vec<Target>,
    backing: Vec<Backing>,
    end: f32,
    seconds_per_beat: f32,
    // Octaves added to the key the player presses so the melody sits on the letter rows. The
    // song still sounds at its original pitch.
    key_shift: u8,
    speed: f32,
    difficulty: Difficulty,
    // Song position when the clock was last started or stopped.
    position: f32,
    started: Option<Instant>,
    // First target that hasn't been judged yet.
    next: usize,
    mistake: Option<Instant>,
}

impl Practice {
    pub fn new(title: String, chart: Chart) -> Self {
        let seconds_per_beat = 60.0 / chart.bpm;
        let (targets, backing) = split_melody(chart);
        let lowest = targets.iter().map(|t| t.key).min().unwrap_or(HIGHEST_KEY);
        let highest = targets.iter().map(|t| t.key).max().unwrap_or(HIGHEST_KEY);
        let mut key_shift = 0;
        while lowest + key_shift < LOWEST_LETTER_KEY && highest + key_shift + 12 <= HIGHEST_KEY {
            key_shift += 12;
        }
        let end = targets
            .iter()
            .map(|t| t.time + t.length)
            .chain(backing.iter().map(|b| b.time + b.length))
            .fold(0.0, f32::max);
        let mut practice = Self {
            title,
            perfect: 0,
            good: 0,
            missed: 0,
            streak: 0,
            targets,
            backing,
            end,
            seconds_per_beat,
            key_shift,
            speed: 1.0,
            difficulty: Difficulty::Normal,
            position: -LEAD_IN_SECONDS,
            started: None,
            next: 0,
            mistake: None,
        };
        practice.assign_auto();
        practice
    }

    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    pub fn next_index(&self) -> usize {
        self.next
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// The key the player presses for a melody note.
    pub fn key_to_press(&self, target: &Target) -> u8 {
        target.key + self.key_shift
    }

    pub fn difficulty(&self) -> Difficulty {
        self.difficulty
    }

    pub fn position(&self) -> f32 {
        self.position
            + self
                .started
                .map_or(0.0, |t| t.elapsed().as_secs_f32() * self.speed)
    }

    pub fn is_playing(&self) -> bool {
        self.started.is_some()
    }

    pub fn is_done(&self) -> bool {
        self.next == self.targets.len() && self.position() > self.end
    }

    pub fn accuracy(&self) -> f32 {
        let judged = self.perfect + self.good + self.missed;
        if judged == 0 {
            return 1.0;
        }
        (self.perfect as f32 + self.good as f32 * 0.5) / judged as f32
    }

    pub fn showing_mistake(&self) -> bool {
        self.mistake.is_some_and(|t| t.elapsed() < MISTAKE_FLASH)
    }

    /// Starts the clock and returns everything that plays by itself from here on: the
    /// accompaniment, and melody notes the difficulty leaves out.
    pub fn play(&mut self) -> Vec<Scheduled> {
        let (position, speed) = (self.position, self.speed);
        self.started = Some(Instant::now());
        let auto = self
            .targets
            .iter()
            .filter(|t| t.auto)
            .map(|t| (t.time, t.length, t.key));
        self.backing
            .iter()
            .map(|b| (b.time, b.length, b.key))
            .chain(auto)
            .filter(|&(time, ..)| time >= position)
            .map(|(time, length, key)| Scheduled {
                delay: (time - position) / speed,
                key,
                length: length / speed,
            })
            .collect()
    }

    pub fn pause(&mut self) {
        self.position = self.position();
        self.started = None;
    }

    /// Changes speed with the clock stopped. Call `play` again to carry on.
    pub fn set_speed(&mut self, speed: f32) {
        self.pause();
        self.speed = speed;
        self.assign_auto();
    }

    /// Changes difficulty with the clock stopped. Call `play` again to carry on.
    pub fn set_difficulty(&mut self, difficulty: Difficulty) {
        self.pause();
        self.difficulty = difficulty;
        self.assign_auto();
    }

    // Picks which upcoming melody notes the player types. Gaps snap to the beat (quarter
    // notes, half notes...) so the typed notes keep the song's rhythm.
    fn assign_auto(&mut self) {
        let min_gap = self.difficulty.min_seconds() * self.speed;
        let gap = if min_gap == 0.0 {
            0.0
        } else {
            NOTE_GRID_BEATS
                .iter()
                .map(|beats| beats * self.seconds_per_beat)
                .find(|&g| g >= min_gap)
                .unwrap_or(NOTE_GRID_BEATS[4] * self.seconds_per_beat)
        };

        let position = self.position;
        let mut last = f32::NEG_INFINITY;
        for target in &mut self.targets {
            if target.time >= position && target.judgement.is_none() {
                // A little slack so notes that land exactly on the grid aren't dropped.
                target.auto = target.time - last < gap - 0.01;
            }
            if !target.auto {
                last = target.time;
            }
        }
        self.skip_judged();
    }

    pub fn restart(&mut self) {
        for target in &mut self.targets {
            target.judgement = None;
        }
        self.perfect = 0;
        self.good = 0;
        self.missed = 0;
        self.streak = 0;
        self.next = 0;
        self.mistake = None;
        self.started = None;
        self.position = -LEAD_IN_SECONDS * self.speed;
        self.assign_auto();
    }

    /// Which keys the song is playing right now: hit melody notes, melody notes the difficulty
    /// plays automatically, and the accompaniment.
    pub fn sounding(&self) -> [bool; 128] {
        let position = self.position();
        let melody = self
            .targets
            .iter()
            .filter(|t| t.auto || matches!(t.judgement, Some(Judgement::Perfect | Judgement::Good)))
            .map(|t| (t.time, t.length, t.key));
        let mut keys = [false; 128];
        for (time, length, key) in self
            .backing
            .iter()
            .map(|b| (b.time, b.length, b.key))
            .chain(melody)
        {
            if time <= position && position < time + length {
                keys[usize::from(key)] = true;
            }
        }
        keys
    }

    /// Whether `key` is a melody note the player could hit right now, or the next one coming.
    pub fn expects(&self, key: u8) -> bool {
        let horizon = self.position() + self.difficulty.good_window() * self.speed;
        let mut pending = self.targets[self.next..].iter().filter(|t| t.is_open());
        pending
            .clone()
            .take_while(|t| t.time <= horizon)
            .any(|t| self.key_to_press(t) == key)
            || pending.next().is_some_and(|t| self.key_to_press(t) == key)
    }

    pub fn press(&mut self, key: u8) -> Press {
        let position = self.position();
        let window = self.difficulty.good_window() * self.speed;
        let found = self.targets[self.next..]
            .iter()
            .take_while(|t| t.time <= position + window)
            .position(|t| {
                t.is_open() && self.key_to_press(t) == key && t.time >= position - window
            });
        let Some(index) = found.map(|i| i + self.next) else {
            self.mistake = Some(Instant::now());
            return Press::Miss;
        };

        let target = &mut self.targets[index];
        let off = (target.time - position).abs() / self.speed;
        let judgement = if off <= PERFECT_WINDOW {
            self.perfect += 1;
            Judgement::Perfect
        } else {
            self.good += 1;
            Judgement::Good
        };
        target.judgement = Some(judgement);
        self.streak += 1;
        let played = Scheduled {
            delay: 0.0,
            key: target.key,
            length: target.length / self.speed,
        };
        self.skip_judged();
        Press::Hit(played)
    }

    /// Marks notes that have gone past the hit window as missed.
    pub fn update(&mut self) {
        let late = self.position() - self.difficulty.good_window() * self.speed;
        let mut missed = 0;
        for target in self.targets[self.next..]
            .iter_mut()
            .take_while(|t| t.time < late)
            .filter(|t| t.is_open())
        {
            target.judgement = Some(Judgement::Miss);
            missed += 1;
        }
        if missed > 0 {
            self.missed += missed;
            self.streak = 0;
        }
        self.skip_judged();
    }

    fn skip_judged(&mut self) {
        while self.targets.get(self.next).is_some_and(|t| !t.is_open()) {
            self.next += 1;
        }
    }
}

/// Splits a chart into melody and accompaniment: the top note of each step is melody, unless
/// it's lower than a melody note that's still sounding, in which case the step is backing.
fn split_melody(chart: Chart) -> (Vec<Target>, Vec<Backing>) {
    let seconds_per_beat = 60.0 / chart.bpm;
    let mut targets: Vec<Target> = Vec::new();
    let mut backing = Vec::new();

    for step in chart.steps {
        let time = step.start * seconds_per_beat;
        let Some(top) = step.notes.iter().map(|n| n.key).max() else {
            continue;
        };
        let under_melody = targets
            .last()
            .is_some_and(|m| top < m.key && time < m.time + m.length);

        for note in step.notes {
            let length = note.beats * seconds_per_beat;
            if !under_melody && note.key == top && targets.last().is_none_or(|m| m.time != time) {
                targets.push(Target {
                    time,
                    length,
                    key: note.key,
                    judgement: None,
                    auto: false,
                });
            } else {
                backing.push(Backing {
                    time,
                    length,
                    key: note.key,
                });
            }
        }
    }
    (targets, backing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::{Note, Step};

    fn chart(steps: &[(f32, &[(u8, f32)])]) -> Chart {
        let steps = steps
            .iter()
            .map(|&(start, notes)| Step {
                start,
                notes: notes
                    .iter()
                    .map(|&(key, beats)| Note { key, beats })
                    .collect(),
            })
            .collect();
        Chart { bpm: 60.0, steps }
    }

    // A two-beat melody note over a chord struck twice, then a second melody note.
    fn song() -> Practice {
        let first: &[(u8, f32)] = &[(62, 2.0), (50, 1.0), (53, 1.0)];
        let chord: &[(u8, f32)] = &[(50, 1.0), (53, 1.0)];
        Practice::new(
            String::new(),
            chart(&[(0.0, first), (1.0, chord), (2.0, &[(64, 1.0)])]),
        )
    }

    fn at(p: &mut Practice, position: f32) {
        p.position = position;
    }

    #[test]
    fn melody_is_hit_and_the_rest_is_backing() {
        let p = song();
        let melody: Vec<_> = p.targets().iter().map(|t| (t.time, t.key)).collect();
        assert_eq!(melody, [(0.0, 62), (2.0, 64)]);
        assert_eq!(p.backing.len(), 4);
    }

    #[test]
    fn play_schedules_backing_at_the_chosen_speed() {
        let mut p = song();
        p.set_speed(0.5);
        at(&mut p, 0.5);
        let scheduled: Vec<_> = p.play().iter().map(|s| (s.delay, s.length)).collect();
        assert_eq!(scheduled, [(1.0, 2.0), (1.0, 2.0)]);
    }

    #[test]
    fn hits_are_judged_by_timing() {
        let mut p = song();
        at(&mut p, 0.02);
        let Press::Hit(note) = p.press(62) else {
            panic!("expected a hit");
        };
        assert_eq!((note.key, note.length), (62, 2.0));
        assert_eq!(p.perfect, 1);

        at(&mut p, 2.12);
        assert!(matches!(p.press(64), Press::Hit(_)));
        assert_eq!((p.good, p.streak), (1, 2));
    }

    #[test]
    fn low_melodies_are_typed_on_the_letter_rows_but_sound_as_written() {
        let low: &[(u8, f32)] = &[(50, 1.0)];
        let high: &[(u8, f32)] = &[(64, 1.0)];
        let mut p = Practice::new(String::new(), chart(&[(0.0, low), (1.0, high)]));
        assert_eq!(p.key_to_press(&p.targets()[0]), 62);

        at(&mut p, 0.0);
        assert!(matches!(p.press(50), Press::Miss));
        let Press::Hit(note) = p.press(62) else {
            panic!("expected a hit");
        };
        assert_eq!(note.key, 50);
    }

    #[test]
    fn sounding_shows_backing_and_hit_notes_but_not_misses() {
        let mut p = song();
        at(&mut p, 0.5);
        let keys = p.sounding();
        assert!(keys[50] && keys[53] && !keys[62]);

        at(&mut p, 0.0);
        p.press(62);
        at(&mut p, 0.5);
        assert!(p.sounding()[62]);
    }

    #[test]
    fn easy_accepts_later_hits() {
        let mut p = song();
        at(&mut p, 0.22);
        assert!(matches!(p.press(62), Press::Miss));
        p.set_difficulty(Difficulty::Easy);
        at(&mut p, 0.22);
        assert!(matches!(p.press(62), Press::Hit(_)));
    }

    #[test]
    fn wrong_or_early_keys_miss() {
        let mut p = song();
        at(&mut p, 0.0);
        assert!(matches!(p.press(64), Press::Miss));
        assert!(p.showing_mistake());
        at(&mut p, -0.5);
        assert!(matches!(p.press(62), Press::Miss));
    }

    #[test]
    fn notes_that_pass_are_missed() {
        let mut p = song();
        at(&mut p, 0.2);
        p.update();
        assert_eq!((p.missed, p.streak, p.next_index()), (1, 0, 1));
        assert!(p.expects(64));
    }

    #[test]
    fn easier_levels_auto_play_some_melody_notes() {
        let eighths: Vec<_> = (0..8).map(|i| (i as f32 * 0.5, 60 + i as u8)).collect();
        let mut p = Practice::new(
            String::new(),
            Chart {
                bpm: 60.0,
                steps: eighths
                    .iter()
                    .map(|&(start, key)| Step {
                        start,
                        notes: vec![Note { key, beats: 0.5 }],
                    })
                    .collect(),
            },
        );
        let typed = |p: &Practice| p.targets().iter().filter(|t| !t.auto).count();

        p.set_difficulty(Difficulty::Hard);
        assert_eq!(typed(&p), 8);
        p.set_difficulty(Difficulty::Easy);
        assert_eq!(typed(&p), 4);
        assert!(!p.targets()[0].auto && p.targets()[1].auto);

        at(&mut p, 0.0);
        assert_eq!(p.play().len(), 4);
    }

    #[test]
    fn restart_resets_the_score() {
        let mut p = song();
        at(&mut p, 0.0);
        p.press(62);
        p.restart();
        assert_eq!((p.perfect, p.next_index()), (0, 0));
        assert!(p.targets().iter().all(|t| t.judgement.is_none()));
        assert_eq!(p.position(), -LEAD_IN_SECONDS);
    }
}
