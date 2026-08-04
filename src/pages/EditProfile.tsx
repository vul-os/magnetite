import { useState, useEffect } from 'react';
import type { ChangeEvent, FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import Layout from '../components/Layout';
import Input from '../components/common/Input';
import Button from '../components/common/Button';
import { api } from '../api/client';
import { useTranslation } from '../i18n/useTranslation';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

interface ProfileFormData {
  username: string;
  bio: string;
  location: string;
  avatar: string;
  [key: string]: unknown;
}

const BLANK_PROFILE: ProfileFormData = { username: '', bio: '', location: '', avatar: '' };

export default function EditProfile() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [formData, setFormData] = useState<ProfileFormData>(BLANK_PROFILE);
  const [avatarFile, setAvatarFile] = useState<File | null>(null);
  const [isSaving, setIsSaving]     = useState(false);
  const [loadError, setLoadError]   = useState<string | null>(null);

  useEffect(() => {
    async function loadProfile() {
      if (USE_MOCKS) {
        try {
          const { mockProfileUser } = await import('../data/mockProfile');
          setFormData({
            username: mockProfileUser.username || '',
            bio:      mockProfileUser.bio      || '',
            location: mockProfileUser.location || '',
            avatar:   mockProfileUser.avatar   || '',
          });
        } catch { /* use blank */ }
        return;
      }

      try {
        const me = await api.auth.me() as Partial<ProfileFormData> | null;
        if (me) {
          setFormData({
            username: me.username || '',
            bio:      me.bio      || '',
            location: me.location || '',
            avatar:   me.avatar   || '',
          });
        }
      } catch (err) {
        setLoadError((err instanceof Error && err.message) || 'Failed to load profile');
      }
    }
    loadProfile();
  }, []);

  const handleChange = (e: ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    const { name, value } = e.target;
    setFormData(prev => ({ ...prev, [name]: value }));
  };

  const handleAvatarChange = (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      setAvatarFile(file);
      const previewUrl = URL.createObjectURL(file);
      setFormData(prev => ({ ...prev, avatar: previewUrl }));
    }
  };

  const handleSubmit = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setIsSaving(true);
    try {
      await api.profile.update(formData);
    } catch { /* optimistic */ }
    setIsSaving(false);
    navigate(`/profile/${formData.username}`);
  };

  return (
    <Layout>
      <div className="edit-profile-page">
        {loadError && (
          <div className="auth-error" role="alert" aria-live="assertive" style={{ marginBottom: '1rem' }}>
            <span className="auth-error-icon" aria-hidden="true">!</span>
            {loadError}
          </div>
        )}
        {/* Header */}
        <header className="settings-page-header edit-profile-header reveal reveal-1">
          <span className="kicker">// YOUR PROFILE</span>
          <h1 className="settings-page-title">{t('account.editProfileTitle')}</h1>
          <p className="settings-page-subtitle">{t('account.editProfileSubtitle')}</p>
        </header>

        <form onSubmit={handleSubmit} className="edit-profile-form reveal reveal-2" aria-label={t('account.editProfileFormLabel')}>
          {/* Avatar */}
          <div className="avatar-upload-section">
            <div className="settings-avatar-wrap" style={{ width: 80, height: 80 }}>
              <img
                src={formData.avatar}
                alt={t('account.avatarPreview')}
                className="settings-avatar"
                style={{ width: 80, height: 80 }}
                loading="lazy"
              />
              <div className="settings-avatar-overlay" aria-hidden="true">{t('account.change')}</div>
              <label className="settings-avatar-input-label" aria-label={t('account.uploadNewAvatar')}>
                <input type="file" accept="image/*" onChange={handleAvatarChange} hidden />
              </label>
            </div>
            <div className="avatar-upload-controls">
              <span className="settings-field-label">{t('account.avatarLabel')}</span>
              <label className="avatar-upload-btn">
                <input type="file" accept="image/*" onChange={handleAvatarChange} hidden />
                {t('account.uploadNewAvatar')}
              </label>
              {avatarFile && (
                <span className="file-name">{avatarFile.name}</span>
              )}
              <span className="settings-avatar-hint">{t('account.avatarHintShort')}</span>
            </div>
          </div>

          {/* Fields */}
          <div className="settings-section" style={{ padding: '1.5rem' }}>
            <div className="settings-grid-2">
              <Input
                label={t('auth.usernameLabel')}
                name="username"
                value={formData.username}
                onChange={handleChange}
                placeholder={t('account.usernamePlaceholder')}
              />
              <Input
                label={t('account.locationLabel')}
                name="location"
                value={formData.location}
                onChange={handleChange}
                placeholder={t('account.locationPlaceholder')}
              />
            </div>

            <div className="input-wrapper">
              <label htmlFor="bio-edit" className="input-label">{t('account.bioLabel')}</label>
              <textarea
                id="bio-edit"
                name="bio"
                value={formData.bio}
                onChange={handleChange}
                placeholder={t('account.bioPlaceholder')}
                className="input-field textarea-field"
                rows={4}
              />
            </div>
          </div>

          <div className="form-actions reveal reveal-3">
            <Button
              type="button"
              variant="secondary"
              onClick={() => navigate(-1)}
            >
              {t('common.cancel')}
            </Button>
            <Button
              type="submit"
              variant="primary"
              // NOTE (pre-existing bug, not fixed here — Button's real loading
              // prop is `isLoading`; `loading` isn't part of ButtonProps and is
              // silently forwarded onto the DOM <button> as an unknown attribute).
              {...({ loading: isSaving } as Record<string, boolean>)}
            >
              {t('account.saveChanges')}
            </Button>
          </div>
        </form>
      </div>
    </Layout>
  );
}
