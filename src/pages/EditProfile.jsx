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
    bio: mockProfileUser.bio,
    location: mockProfileUser.location,
    avatar: mockProfileUser.avatar,
  });
  const [avatarFile, setAvatarFile] = useState(null);
  const [isSaving, setIsSaving] = useState(false);

  // Load real profile data on mount
  useEffect(() => {
    api.auth.me().then(me => {
      if (me) {
        setFormData(prev => ({
          ...prev,
          username: me.username || prev.username,
          bio: me.bio || prev.bio,
          location: me.location || prev.location,
          avatar: me.avatar || prev.avatar,
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
    } catch { /* optimistic — proceed */ }
    setIsSaving(false);
    navigate(`/profile/${formData.username}`);
  };

  return (
    <Layout>
      <div className="edit-profile-page">
        <header className="page-header">
          <h1>Edit Profile</h1>
        </header>

        <form onSubmit={handleSubmit} className="edit-profile-form">
          <div className="avatar-upload-section">
            <div className="avatar-preview">
              <img src={formData.avatar} alt="Avatar" loading="lazy" />
            </div>
            <div className="avatar-upload-controls">
              <label className="avatar-upload-btn">
                <input
                  type="file"
                  accept="image/*"
                  onChange={handleAvatarChange}
                  hidden
                />
                Upload New Avatar
              </label>
              {avatarFile && <span className="file-name">{avatarFile.name}</span>}
            </div>
          </div>

          <Input
            label="Username"
            name="username"
            value={formData.username}
            onChange={handleChange}
            placeholder="Enter your username"
          />

          <div className="input-wrapper">
            <label htmlFor="bio" className="input-label">Bio</label>
            <textarea
              id="bio"
              name="bio"
              value={formData.bio}
              onChange={handleChange}
              placeholder="Tell us about yourself"
              className="input-field textarea-field"
              rows={4}
            />
          </div>

          <Input
            label="Location"
            name="location"
            value={formData.location}
            onChange={handleChange}
            placeholder="City, Country"
          />

          <div className="form-actions">
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
