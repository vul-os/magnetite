export const schemas = {
  email: /^[^\s@]+@[^\s@]+\.[^\s@]+$/,

  password: {
    minLength: 8,
    hasUpperCase: /[A-Z]/,
    hasLowerCase: /[a-z]/,
    hasNumber: /[0-9]/,
    hasSpecial: /[!@#$%^&*(),.?":{}|<>]/,
  },

  username: {
    minLength: 3,
    maxLength: 20,
    pattern: /^[a-zA-Z0-9_]+$/,
  },
};

export function validateEmail(email) {
  if (!email) return 'Email is required';
  if (!schemas.email.test(email)) return 'Invalid email address';
  return null;
}

export function validatePassword(password) {
  if (!password) return 'Password is required';
  if (password.length < schemas.password.minLength)
    return `Password must be at least ${schemas.password.minLength} characters`;
  if (!schemas.password.hasUpperCase.test(password))
    return 'Password must contain at least one uppercase letter';
  if (!schemas.password.hasLowerCase.test(password))
    return 'Password must contain at least one lowercase letter';
  if (!schemas.password.hasNumber.test(password))
    return 'Password must contain at least one number';
  if (!schemas.password.hasSpecial.test(password))
    return 'Password must contain at least one special character';
  return null;
}

export function validateUsername(username) {
  if (!username) return 'Username is required';
  if (username.length < schemas.username.minLength)
    return `Username must be at least ${schemas.username.minLength} characters`;
  if (username.length > schemas.username.maxLength)
    return `Username must be at most ${schemas.username.maxLength} characters`;
  if (!schemas.username.pattern.test(username))
    return 'Username can only contain letters, numbers, and underscores';
  return null;
}
