import { isEnabled, type FeatureFlag } from '../utils/featureFlags';

export function useFeatureFlag(flag: FeatureFlag) {
  return isEnabled(flag);
}
