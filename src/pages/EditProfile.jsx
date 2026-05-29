import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import Layout from '../components/Layout';
import Input from '../components/common/Input';
import Button from '../components/common/Button';
import { mockProfileUser } from '../data/mockProfile';
import { api } from '../api/client';

export default function EditProfile() {
  const navigate = useNavigate();
  const [formData, setFormData] = useState({
    username: mockProfileUser.username,
    bio:      mockProfileUser.bio,
    location: mockProfileUser.location,
    avatar:   mockProfileUser.avatar,
  });
  const [avatarFile, setAvatarFile] = useState(null);
  const [isSaving, setIsSaving]     = useState(false);

  useEffect(() => {
    api.auth.me().then(me => {
      if (me) {
        setFormData(prev => ({
          ...prev,
          username: me.username || prev.username,
          bio:      me.bio      || prev.bio,
          location: me.location || prev.location,
          avatar:   me.avatar   || prev.avatar,
        }));
      }
    }).catch(() => { /* use mock */ });
  }, []);

  const handleChange = (e) => {
    const { name, value } = e.target;
    setFormData(prev => ({ ...prev, [name]: value }));
  };

  const handleAvatarChange = (e) => {
    const file = e.target.files[0];
    if (file) {
      setAvatarFile(file);
      const previewUrl = URL.createObjectURL(file);
      setFormData(prev => ({ ...prev, avatar: previewUrl }));
    }
  };

  const handleSubmit = async (e) => {
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
        {/* Header */}
        <header className="settings-page-header edit-profile-header reveal reveal-1">
          <span className="kicker">// YOUR PROFILE</span>
          <h1 className="settings-page-title">Edit Profile</h1>
          <p className="settings-page-subtitle">
            This information is visible to other players and developers.
          </p>
        </header>

        <form onSubmit={handleSubmit} className="edit-profile-form reveal reveal-2">
          {/* Avatar */}
          <div className="avatar-upload-section">
            <div className="settings-avatar-wrap" style={{ width: 80, height: 80 }}>
              <img
                src={formData.avatar}
                alt="Your avatar preview"
                className="settings-avatar"
                style={{ width: 80, height: 80 }}
                loading="lazy"
              />
              <div className="settings-avatar-overlay" aria-hidden="true">Change</div>
              <label className="settings-avatar-input-label" aria-label="Upload new avatar">
                <input type="file" accept="image/*" onChange={handleAvatarChange} hidden />
              </label>
            </div>
            <div className="avatar-upload-controls">
              <span className="settings-field-label">AVATAR</span>
              <label className="avatar-upload-btn">
                <input type="file" accept="image/*" onChange={handleAvatarChange} hidden />
                Upload New Avatar
              </label>
              {avatarFile && (
                <span className="file-name">{avatarFile.name}</span>
              )}
              <span className="settings-avatar-hint">JPG or PNG · 200×200 px recommended</span>
            </div>
          </div>

          {/* Fields */}
          <div className="settings-section" style={{ padding: '1.5rem' }}>
            <div className="settings-grid-2">
              <Input
                label="Username"
                name="username"
                value={formData.username}
                onChange={handleChange}
                placeholder="Enter your username"
              />
              <Input
                label="Location"
                name="location"
                value={formData.location}
                onChange={handleChange}
                placeholder="City, Country"
              />
            </div>

            <div className="input-wrapper">
              <label htmlFor="bio-edit" className="input-label">Bio</label>
              <textarea
                id="bio-edit"
                name="bio"
                value={formData.bio}
                onChange={handleChange}
                placeholder="Tell us about yourself and your games…"
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
              Cancel
            </Button>
            <Button
              type="submit"
              variant="primary"
              loading={isSaving}
            >
              Save Changes
            </Button>
          </div>
        </form>
      </div>
    </Layout>
  );
}
