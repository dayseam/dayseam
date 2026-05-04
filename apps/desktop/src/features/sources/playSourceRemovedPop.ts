/** Short percussive cue when a source chip finishes its delete exit
 *  animation. Implemented with Web Audio (no bundled samples) so
 *  licensing stays simple; skipped when the user prefers reduced
 *  motion so audio never substitutes for a visual affordance they
 *  asked the platform to minimise. */
export function playSourceRemovedPop(): void {
  if (typeof window === "undefined") return;
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    return;
  }
  try {
    const AC =
      window.AudioContext ||
      (window as unknown as { webkitAudioContext?: typeof AudioContext })
        .webkitAudioContext;
    if (!AC) return;
    const ctx = new AC();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "sine";
    osc.frequency.setValueAtTime(620, ctx.currentTime);
    gain.gain.setValueAtTime(0.0001, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.11, ctx.currentTime + 0.018);
    gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.085);
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.onended = () => {
      void ctx.close();
    };
    osc.start();
    osc.stop(ctx.currentTime + 0.09);
  } catch {
    // Missing AudioContext, autoplay policy, etc.
  }
}
