//! Embers, flames and heat behind the Home page (spec 4.3 item 5).

use iced::mouse;
use iced::widget::canvas::{self, Geometry, Path};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, Vector};

use crate::ui::app::Message;

// Task 36 adds Shader
/// The fire layer.
#[derive(Debug)]
pub enum Fire {
    Canvas(FireCanvas),
}

impl Default for Fire {
    fn default() -> Self {
        Fire::Canvas(FireCanvas::default())
    }
}

impl Fire {
    pub fn set_intensity(&mut self, intensity: f32) {
        match self {
            Fire::Canvas(fire) => fire.set_intensity(intensity),
        }
    }

    pub fn tick(&mut self, dt: f32, size: Size) {
        match self {
            Fire::Canvas(fire) => fire.tick(dt, size),
        }
    }
}

/// How much fire there is: 1.0 on the splash, 0.8 on a focused Home, 0.3 unfocused, none when
/// hidden or when effects are off.
pub fn intensity_for(splash: bool, focused: bool, visible: bool, effects: bool) -> f32 {
    if !effects || !visible {
        0.0
    } else if splash {
        1.0
    } else if focused {
        0.8
    } else {
        0.3
    }
}

/// Particles alive at full intensity.
const FULL_COUNT: f32 = 120.0;
/// Particles born per tick at most, so the fire fills in over a second instead of in one burst.
const MAX_SPAWN_PER_TICK: usize = 6;
/// Share of the spawns that are flames.
const FLAME_SHARE: f32 = 0.22;
/// Height of the spawn line on the right edge, as a share of the window height.
const RISE: f32 = 0.22;
/// Extra life on the right edge.
const EXTRA_LIFE: f32 = 0.7;

/// Small deterministic generator; the fire needs variety, not quality.
#[derive(Debug, Clone)]
struct Xorshift32 {
    state: u32,
}

impl Xorshift32 {
    fn new(seed: u32) -> Self {
        Xorshift32 {
            state: if seed == 0 { 0x9e37_79b9 } else { seed },
        }
    }

    /// Uniform in `[0, 1)`.
    fn next_f32(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next_f32()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleKind {
    Ember,
    Flame,
}

#[derive(Debug, Clone)]
pub struct Particle {
    pos: Point,
    vel: Vector,
    life: f32,
    max_life: f32,
    kind: ParticleKind,
    radius: f32,
}

impl Particle {
    /// Share of the life left, 1 at birth and 0 at death.
    fn remaining(&self) -> f32 {
        (1.0 - self.life / self.max_life).clamp(0.0, 1.0)
    }
}

/// The canvas fire: particles, a geometry cache and its own random source.
#[derive(Debug)]
pub struct FireCanvas {
    particles: Vec<Particle>,
    cache: canvas::Cache,
    intensity: f32,
    rng: Xorshift32,
    size: Size,
    /// Where every particle was born (x, y), for the tests.
    #[cfg(test)]
    spawn_log: Vec<(f32, f32)>,
}

impl Default for FireCanvas {
    fn default() -> Self {
        FireCanvas::seeded(0x5eed_f1e5)
    }
}

impl FireCanvas {
    pub fn seeded(seed: u32) -> Self {
        FireCanvas {
            particles: Vec::new(),
            cache: canvas::Cache::new(),
            intensity: 0.0,
            rng: Xorshift32::new(seed),
            size: Size::ZERO,
            #[cfg(test)]
            spawn_log: Vec::new(),
        }
    }

    pub fn set_intensity(&mut self, intensity: f32) {
        self.intensity = intensity.clamp(0.0, 1.0);
    }

    fn spawn(&mut self, size: Size) {
        let x = self.rng.next_f32() * size.width;
        let line = size.height - RISE * size.height * (x / size.width);
        let y = line + self.rng.range(-6.0, 6.0);
        let flame = self.rng.next_f32() < FLAME_SHARE;
        let (kind, rise, radius, base_life) = if flame {
            (
                ParticleKind::Flame,
                self.rng.range(30.0, 80.0),
                self.rng.range(14.0, 34.0),
                self.rng.range(1.2, 2.2),
            )
        } else {
            (
                ParticleKind::Ember,
                self.rng.range(60.0, 160.0),
                self.rng.range(1.0, 2.5),
                self.rng.range(2.5, 4.5),
            )
        };
        let drift = self.rng.range(-15.0, 15.0);
        #[cfg(test)]
        self.spawn_log.push((x, y));
        self.particles.push(Particle {
            pos: Point::new(x, y),
            vel: Vector::new(drift, -rise),
            life: 0.0,
            max_life: base_life * (1.0 + EXTRA_LIFE * x / size.width),
            kind,
            radius,
        });
    }

    /// Advances the fire by `dt` seconds in a window of `size`.
    pub fn tick(&mut self, dt: f32, size: Size) {
        self.size = size;
        for particle in &mut self.particles {
            particle.life += dt;
            particle.pos += particle.vel * dt;
        }
        self.particles
            .retain(|p| p.life < p.max_life && p.pos.y > -p.radius);
        if size.width > 0.0 && size.height > 0.0 {
            let target = (FULL_COUNT * self.intensity) as usize;
            let mut born = 0;
            while self.particles.len() < target && born < MAX_SPAWN_PER_TICK {
                self.spawn(size);
                born += 1;
            }
        }
        self.cache.clear();
    }

    #[cfg(test)]
    pub fn take_spawned(&mut self) -> Vec<(f32, f32)> {
        std::mem::take(&mut self.spawn_log)
    }
}

impl canvas::Program<Message> for FireCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            for particle in &self.particles {
                let left = particle.remaining();
                match particle.kind {
                    ParticleKind::Ember => {
                        let color = Color::from_rgba(1.0, 0.45 + 0.3 * left, 0.12, 0.9 * left);
                        frame.fill(&Path::circle(particle.pos, particle.radius), color);
                    }
                    ParticleKind::Flame => {
                        // The canvas has no radial gradient: stacked discs fake the falloff.
                        for (scale, alpha) in [(1.0, 0.05), (0.65, 0.07), (0.35, 0.10)] {
                            let color =
                                Color::from_rgba(0.95, 0.25 + 0.2 * left, 0.05, alpha * left);
                            frame.fill(&Path::circle(particle.pos, particle.radius * scale), color);
                        }
                    }
                }
            }
        });
        vec![geometry]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Size;

    #[test]
    fn intensity_rules() {
        assert_eq!(intensity_for(true, true, true, true), 1.0);
        assert_eq!(intensity_for(false, true, true, true), 0.8);
        assert_eq!(intensity_for(false, false, true, true), 0.3);
        assert_eq!(intensity_for(true, true, false, true), 0.0);
        assert_eq!(intensity_for(true, true, true, false), 0.0);
    }

    #[test]
    fn particles_spawn_higher_on_the_right() {
        let size = Size::new(1000.0, 800.0);
        let mut fire = FireCanvas::seeded(12345);
        fire.set_intensity(1.0);
        let mut left = Vec::new();
        let mut right = Vec::new();
        let mut spawned = 0;
        for _ in 0..600 {
            fire.tick(0.033, size);
            for y_x in fire.take_spawned() {
                spawned += 1;
                if y_x.0 < 0.2 * size.width {
                    left.push(y_x.1);
                } else if y_x.0 > 0.8 * size.width {
                    right.push(y_x.1);
                }
            }
            if spawned >= 200 && !left.is_empty() && !right.is_empty() {
                break;
            }
        }
        assert!(spawned >= 200);
        let avg = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        assert!(avg(&right) < avg(&left));
    }
}
