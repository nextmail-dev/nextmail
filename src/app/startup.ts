interface AnimationFrameScheduler {
  requestAnimationFrame(callback: FrameRequestCallback): number;
  cancelAnimationFrame(handle: number): void;
}

export function afterFirstPaint(
  callback: () => void,
  scheduler: AnimationFrameScheduler = window,
) {
  let secondFrame = 0;
  const firstFrame = scheduler.requestAnimationFrame(() => {
    secondFrame = scheduler.requestAnimationFrame(callback);
  });
  return () => {
    scheduler.cancelAnimationFrame(firstFrame);
    if (secondFrame) scheduler.cancelAnimationFrame(secondFrame);
  };
}
