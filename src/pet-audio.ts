import type { PetState } from "./pet-model";

export type SoundCue = "chirp" | "tumble";

export class SoundGate {
  private enabled = false;
  private previousState: PetState = "idle";
  private lastClickAt = Number.NEGATIVE_INFINITY;

  constructor(private readonly clickSuppressionMs = 750) {}

  setEnabled(enabled: boolean): void {
    this.enabled = enabled;
  }

  clicked(now: number): SoundCue | null {
    this.lastClickAt = now;
    return this.enabled ? "chirp" : null;
  }

  stateChanged(nextState: PetState, now: number): SoundCue | null {
    const enteredTumbling = nextState === "tumbling" && this.previousState !== "tumbling";
    this.previousState = nextState;
    if (!this.enabled || !enteredTumbling) return null;
    return now - this.lastClickAt >= this.clickSuppressionMs ? "tumble" : null;
  }
}

export class PetAudioPlayer {
  private readonly sounds = new Map<SoundCue, HTMLAudioElement>();

  constructor() {
    this.prepare("chirp", "/assets/audio/chirp.wav");
    this.prepare("tumble", "/assets/audio/tumble.wav");
  }

  play(cue: SoundCue | null): void {
    if (!cue) return;
    const audio = this.sounds.get(cue);
    if (!audio) return;
    try {
      this.stopAll();
      audio.currentTime = 0;
      const playback = audio.play();
      void playback.catch(() => undefined);
    } catch {
      // Audio is optional feedback. Media and autoplay failures must not affect interaction.
    }
  }

  stopAll(): void {
    for (const audio of this.sounds.values()) {
      try {
        audio.pause();
        audio.currentTime = 0;
      } catch {
        // Ignore unavailable or detached audio devices.
      }
    }
  }

  private prepare(cue: SoundCue, source: string): void {
    try {
      const audio = new Audio(source);
      audio.preload = "auto";
      this.sounds.set(cue, audio);
    } catch {
      // Keep the desktop pet usable when the WebView has no audio device.
    }
  }
}
