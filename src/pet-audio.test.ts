import { describe, expect, it } from "vitest";
import { SoundGate } from "./pet-audio";

describe("SoundGate", () => {
  it("stays silent until sound is explicitly enabled", () => {
    const gate = new SoundGate();
    expect(gate.clicked(100)).toBeNull();
    expect(gate.stateChanged("tumbling", 900)).toBeNull();
  });

  it("plays chirp for clicks and suppresses the resulting tumble cue", () => {
    const gate = new SoundGate(750);
    gate.setEnabled(true);
    expect(gate.clicked(1_000)).toBe("chirp");
    expect(gate.stateChanged("tumbling", 1_050)).toBeNull();
    expect(gate.stateChanged("tumbling", 2_000)).toBeNull();
  });

  it("plays tumble once when entering it automatically", () => {
    const gate = new SoundGate(750);
    gate.setEnabled(true);
    expect(gate.stateChanged("walking", 2_000)).toBeNull();
    expect(gate.stateChanged("tumbling", 3_000)).toBe("tumble");
    expect(gate.stateChanged("tumbling", 3_100)).toBeNull();
    expect(gate.stateChanged("idle", 4_000)).toBeNull();
    expect(gate.stateChanged("tumbling", 5_000)).toBe("tumble");
  });

  it("allows automatic tumble after the click suppression window", () => {
    const gate = new SoundGate(750);
    gate.setEnabled(true);
    gate.clicked(1_000);
    gate.stateChanged("idle", 1_100);
    expect(gate.stateChanged("tumbling", 1_750)).toBe("tumble");
  });
});
