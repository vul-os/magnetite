import { isEnabled } from '../utils/featureFlags';

export function useFeatureFlag(flag) {
  return isEnabled(flag);
}
