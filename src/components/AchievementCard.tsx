export default function AchievementCard({ achievement }) {
  const isUnlocked = achievement.unlockedAt !== null;
  const progressPercent = Math.min((achievement.progress / achievement.maxProgress) * 100, 100);

  return (
    <div
      className={`achievement-card ${isUnlocked ? 'unlocked' : 'locked'}`}
      aria-label={`${achievement.name} — ${isUnlocked ? 'unlocked' : 'locked'}`}
    >
      <div className="achievement-icon" aria-hidden="true">
        {isUnlocked ? achievement.icon : '🔒'}
      </div>

      <div className="achievement-content">
        <h4>{achievement.name}</h4>
        <p>{achievement.description}</p>

        {!isUnlocked && (
          <div className="progress-container">
            <div
              className="progress-bar"
              role="progressbar"
              aria-valuenow={achievement.progress}
              aria-valuemin={0}
              aria-valuemax={achievement.maxProgress}
              aria-label={`Progress: ${achievement.progress} of ${achievement.maxProgress}`}
            >
              <div
                className="progress-fill"
                style={{ width: `${progressPercent}%` }}
              />
            </div>
            <span className="progress-text">
              {achievement.progress}/{achievement.maxProgress}
            </span>
          </div>
        )}

        {isUnlocked && achievement.unlockedAt && (
          <span className="unlock-date">
            Unlocked {new Date(achievement.unlockedAt).toLocaleDateString()}
          </span>
        )}
      </div>
    </div>
  );
}
