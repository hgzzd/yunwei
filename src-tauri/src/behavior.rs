use crate::model::{BubblePayload, Facing, PetState, StatePayload};

const ACTION_MIN_MS: u64 = 15_000;
const ACTION_MAX_MS: u64 = 45_000;
const BUBBLE_MIN_MS: u64 = 180_000;
const BUBBLE_MAX_MS: u64 = 480_000;
const BUBBLE_DURATION_MS: u64 = 4_000;
const FRAME_MS: u64 = 1_000 / 12;

const QUIPS: &[&str] = &[
    "别卷啦",
    "摸鱼被抓",
    "尾巴有点云",
    "我先躺会儿",
    "今天也很行",
    "你忙，我巡逻",
    "刚刚谁在看我",
    "伸个懒腰",
];

#[derive(Debug)]
pub struct BehaviorEngine {
    state: PetState,
    facing: Facing,
    frame_elapsed_ms: u64,
    state_elapsed_ms: u64,
    action_due_ms: u64,
    bubble_due_ms: u64,
    bubble_remaining_ms: u64,
    rng: Lcg,
}

impl BehaviorEngine {
    pub fn new(seed: u64) -> Self {
        let mut engine = Self {
            state: PetState::Idle,
            facing: Facing::Left,
            frame_elapsed_ms: 0,
            state_elapsed_ms: 0,
            action_due_ms: ACTION_MIN_MS,
            bubble_due_ms: BUBBLE_MIN_MS,
            bubble_remaining_ms: 0,
            rng: Lcg::new(seed),
        };
        engine.action_due_ms = engine.rng.range(ACTION_MIN_MS, ACTION_MAX_MS);
        engine.bubble_due_ms = engine.rng.range(BUBBLE_MIN_MS, BUBBLE_MAX_MS);
        engine
    }

    pub fn payload(&self) -> StatePayload {
        StatePayload {
            state: self.state,
            facing: self.facing,
            frame: ((self.frame_elapsed_ms / FRAME_MS) % u64::from(self.state.frames())) as u16,
        }
    }

    pub fn state(&self) -> PetState {
        self.state
    }

    pub fn facing(&self) -> Facing {
        self.facing
    }

    pub fn reverse(&mut self) {
        self.facing.reverse();
    }

    pub fn begin_drag(&mut self) {
        self.set_state(PetState::Dragged);
    }

    pub fn end_drag(&mut self) {
        self.set_state(PetState::Idle);
        self.action_due_ms = self.rng.range(ACTION_MIN_MS, ACTION_MAX_MS);
    }

    pub fn clicked(&mut self) -> BubblePayload {
        self.set_state(PetState::Tumbling);
        self.action_due_ms = 2_000;
        self.start_bubble("抓到你啦".into())
    }

    pub fn tick(&mut self, elapsed_ms: u64) -> TickResult {
        self.frame_elapsed_ms = self.frame_elapsed_ms.saturating_add(elapsed_ms);
        self.state_elapsed_ms = self.state_elapsed_ms.saturating_add(elapsed_ms);

        if self.state != PetState::Dragged && self.state_elapsed_ms >= self.action_due_ms {
            self.choose_next_state();
        }

        let bubble = if self.bubble_remaining_ms > 0 {
            let before = self.bubble_remaining_ms;
            self.bubble_remaining_ms = self.bubble_remaining_ms.saturating_sub(elapsed_ms);
            (before > 0 && self.bubble_remaining_ms == 0).then_some(BubblePayload {
                visible: false,
                text: None,
                duration_ms: 0,
            })
        } else if self.state != PetState::Dragged {
            self.bubble_due_ms = self.bubble_due_ms.saturating_sub(elapsed_ms);
            if self.bubble_due_ms == 0 {
                let index = self.rng.range(0, (QUIPS.len() - 1) as u64) as usize;
                Some(self.start_bubble(QUIPS[index].into()))
            } else {
                None
            }
        } else {
            None
        };

        TickResult {
            state: self.payload(),
            bubble,
        }
    }

    fn set_state(&mut self, state: PetState) {
        self.state = state;
        self.frame_elapsed_ms = 0;
        self.state_elapsed_ms = 0;
    }

    fn choose_next_state(&mut self) {
        const STATES: &[PetState] = &[
            PetState::Idle,
            PetState::Walking,
            PetState::Running,
            PetState::Sitting,
            PetState::Sleeping,
            PetState::Stretching,
            PetState::Tumbling,
        ];
        let index = self.rng.range(0, (STATES.len() - 1) as u64) as usize;
        self.set_state(STATES[index]);
        if self.rng.range(0, 1) == 1 {
            self.facing.reverse();
        }
        self.action_due_ms = self.rng.range(ACTION_MIN_MS, ACTION_MAX_MS);
    }

    fn start_bubble(&mut self, text: String) -> BubblePayload {
        self.bubble_remaining_ms = BUBBLE_DURATION_MS;
        self.bubble_due_ms = self.rng.range(BUBBLE_MIN_MS, BUBBLE_MAX_MS);
        BubblePayload {
            visible: true,
            text: Some(text),
            duration_ms: BUBBLE_DURATION_MS,
        }
    }
}

#[derive(Debug)]
pub struct TickResult {
    pub state: StatePayload,
    pub bubble: Option<BubblePayload>,
}

#[derive(Debug)]
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn range(&mut self, min: u64, max: u64) -> u64 {
        debug_assert!(min <= max);
        min + self.next() % (max - min + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_deadlines_stay_inside_contract() {
        for seed in 1..100 {
            let engine = BehaviorEngine::new(seed);
            assert!((ACTION_MIN_MS..=ACTION_MAX_MS).contains(&engine.action_due_ms));
            assert!((BUBBLE_MIN_MS..=BUBBLE_MAX_MS).contains(&engine.bubble_due_ms));
        }
    }

    #[test]
    fn dragging_blocks_random_transitions() {
        let mut engine = BehaviorEngine::new(7);
        engine.begin_drag();
        engine.tick(ACTION_MAX_MS * 2);
        assert_eq!(engine.state(), PetState::Dragged);
        engine.end_drag();
        assert_eq!(engine.state(), PetState::Idle);
    }

    #[test]
    fn click_has_a_fixed_visible_bubble_and_tumble() {
        let mut engine = BehaviorEngine::new(9);
        let bubble = engine.clicked();
        assert_eq!(engine.state(), PetState::Tumbling);
        assert!(bubble.visible);
        assert_eq!(bubble.duration_ms, 4_000);

        let result = engine.tick(4_000);
        assert!(!result.bubble.unwrap().visible);
    }

    #[test]
    fn animation_wraps_at_the_states_frame_count() {
        let mut engine = BehaviorEngine::new(5);
        let initial = engine.payload();
        let advanced = engine.tick(FRAME_MS * u64::from(PetState::Idle.frames()));
        assert_eq!(initial.frame, advanced.state.frame);
    }
}
