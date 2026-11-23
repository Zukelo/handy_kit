export interface ClosableView {
  dirty(): boolean;
  canClose(): Promise<boolean>;
  dispose(): void;
}

/** Confirm every dirty view before disposing any of them. */
export async function closeViews(
  views: ClosableView[],
  isCurrent: () => boolean = () => true,
  commit: () => void = () => {},
): Promise<boolean> {
  for (const view of views) {
    if (view.dirty() && !(await view.canClose())) return false;
    if (!isCurrent()) return false;
  }
  if (!isCurrent()) return false;
  for (const view of views) view.dispose();
  commit();
  return true;
}
